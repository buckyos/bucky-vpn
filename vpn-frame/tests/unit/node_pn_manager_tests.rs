use super::{NodePnInfoState, NodePnManager};
use crate::pn_server_info::PnServerEndpoint;
use crate::server::NodeId;
use crate::{ClientProxyNodeInfo, NodeNetworkPnInfo};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

fn node(byte: u8) -> NodeId {
    NodeId::from([byte; 32].as_slice())
}

fn endpoint(last_octet: u8, port: u16) -> PnServerEndpoint {
    PnServerEndpoint::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, last_octet)), port)
}

fn proxy(proxy_byte: u8, name: &str, endpoints: Vec<PnServerEndpoint>) -> ClientProxyNodeInfo {
    ClientProxyNodeInfo {
        proxy_id: node(proxy_byte),
        name: Some(name.to_owned()),
        endpoints,
    }
}

fn assignment(network_id: u64, proxy: Option<ClientProxyNodeInfo>) -> NodeNetworkPnInfo {
    NodeNetworkPnInfo { network_id, proxy }
}

fn fixed_clock(initial: u32) -> (Arc<AtomicU32>, Arc<NodePnManager>) {
    let now = Arc::new(AtomicU32::new(initial));
    let test_now = now.clone();
    let manager = NodePnManager::with_now_seconds(move || test_now.load(Ordering::SeqCst));
    (now, manager)
}

#[test]
fn clock_is_captured_once_at_startup_and_read_only_for_real_changes() {
    let now = Arc::new(AtomicU32::new(1_700_000_000));
    let calls = Arc::new(AtomicUsize::new(0));
    let test_now = now.clone();
    let test_calls = calls.clone();
    let manager = NodePnManager::with_now_seconds(move || {
        test_calls.fetch_add(1, Ordering::SeqCst);
        test_now.load(Ordering::SeqCst)
    });
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let node_id = node(30);
    let networks = vec![assignment(1, None)];
    assert_eq!(
        manager.update_node_pn_info(&node_id, networks.clone()),
        (1_700_000_000, true)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    now.store(1_700_000_100, Ordering::SeqCst);
    assert_eq!(
        manager.update_node_pn_info(&node_id, networks),
        (1_700_000_000, false)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    assert_eq!(
        manager.update_node_pn_info(&node_id, vec![assignment(2, None)]),
        (1_700_000_100, true)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn first_non_empty_reconstruction_uses_sn_startup_seconds() {
    let (now, manager) = fixed_clock(1_723_456_789);
    now.store(1_723_456_799, Ordering::SeqCst);
    let node_id = node(1);
    let networks = vec![assignment(
        7,
        Some(proxy(90, "pn-a", vec![endpoint(10, 3624)])),
    )];

    assert_eq!(
        manager.update_node_pn_info(&node_id, networks.clone()),
        (1_723_456_789, true)
    );
    assert_eq!(
        manager.get_node_pn_info(&node_id),
        Some(NodePnInfoState {
            node_id,
            version: 1_723_456_789,
            networks,
        })
    );
}

#[test]
fn first_empty_reconstruction_uses_the_same_sn_startup_default() {
    let (now, manager) = fixed_clock(1_723_456_700);
    let node_id = node(2);
    let other_node_id = node(20);

    assert_eq!(
        manager.update_node_pn_info(&node_id, Vec::new()),
        (1_723_456_700, true)
    );
    now.store(1_723_456_799, Ordering::SeqCst);
    assert_eq!(
        manager.update_node_pn_info(&other_node_id, Vec::new()),
        (1_723_456_700, true)
    );
    assert_eq!(
        manager.get_node_pn_info(&node_id).unwrap().networks,
        Vec::new()
    );
}

#[test]
fn identical_canonical_content_retains_version_when_clock_moves() {
    let (now, manager) = fixed_clock(10_000);
    let node_id = node(3);
    let networks = vec![assignment(1, None)];
    assert_eq!(
        manager.update_node_pn_info(&node_id, networks.clone()),
        (10_000, true)
    );

    now.store(20_000, Ordering::SeqCst);
    assert_eq!(
        manager.update_node_pn_info(&node_id, networks),
        (10_000, false)
    );
}

#[test]
fn network_order_is_canonical_and_duplicate_ids_keep_the_last_value() {
    let (_now, manager) = fixed_clock(30_000);
    let node_id = node(4);
    let older = proxy(91, "older", vec![endpoint(11, 3624)]);
    let selected = proxy(92, "selected", vec![endpoint(12, 3625)]);

    assert_eq!(
        manager.update_node_pn_info(
            &node_id,
            vec![
                assignment(9, None),
                assignment(2, Some(older)),
                assignment(2, Some(selected.clone())),
            ],
        ),
        (30_000, true)
    );
    let state = manager.get_node_pn_info(&node_id).unwrap();
    assert_eq!(
        state
            .networks
            .iter()
            .map(|network| network.network_id)
            .collect::<Vec<_>>(),
        vec![2, 9]
    );
    assert_eq!(state.networks[0].proxy.as_ref(), Some(&selected));

    assert_eq!(
        manager.update_node_pn_info(
            &node_id,
            vec![
                assignment(2, None),
                assignment(9, None),
                assignment(2, Some(selected)),
            ],
        ),
        (30_000, false)
    );
}

#[test]
fn every_client_visible_assignment_field_and_membership_transition_uses_current_seconds() {
    let (now, manager) = fixed_clock(40_000);
    let node_id = node(5);
    let original = proxy(93, "pn", vec![endpoint(13, 3624)]);
    let initial = vec![assignment(1, Some(original.clone()))];
    assert_eq!(
        manager.update_node_pn_info(&node_id, initial.clone()),
        (40_000, true)
    );

    let transitions = [
        vec![assignment(
            1,
            Some(proxy(93, "renamed", vec![endpoint(13, 3624)])),
        )],
        vec![assignment(
            1,
            Some(proxy(93, "renamed", vec![endpoint(14, 4624)])),
        )],
        vec![assignment(
            1,
            Some(proxy(94, "replacement", vec![endpoint(15, 5624)])),
        )],
        vec![assignment(1, None)],
        vec![assignment(1, Some(original.clone()))],
        vec![assignment(1, Some(original.clone())), assignment(2, None)],
        vec![assignment(1, Some(original.clone()))],
    ];

    for (index, networks) in transitions.into_iter().enumerate() {
        let raw_now = 41_000 + index as u32;
        now.store(raw_now, Ordering::SeqCst);
        let (version, changed) = manager.update_node_pn_info(&node_id, networks);
        assert!(changed, "transition {index} must be visible");
        assert_eq!(version, raw_now);
    }
}

#[test]
fn same_second_and_clock_rollback_changes_use_raw_seconds() {
    let (now, manager) = fixed_clock(50_000);
    let node_id = node(6);
    assert_eq!(
        manager.update_node_pn_info(&node_id, vec![assignment(1, None)]),
        (50_000, true)
    );

    assert_eq!(
        manager.update_node_pn_info(&node_id, vec![assignment(1, Some(proxy(95, "pn", vec![])))]),
        (50_000, true)
    );

    now.store(1_000, Ordering::SeqCst);
    assert_eq!(
        manager.update_node_pn_info(&node_id, vec![assignment(1, None)]),
        (1_000, true)
    );
}

#[test]
fn u32_max_second_is_stored_without_wrapping() {
    let node_id = node(7);
    let mut state = NodePnInfoState::new(node_id, vec![assignment(1, None)], 0);

    assert!(state.update_networks(vec![assignment(2, None)], u32::MAX,));
    assert_eq!(state.version, u32::MAX);
}

#[test]
fn versions_are_owned_independently_per_client_node() {
    let (now, manager) = fixed_clock(60_000);
    let node_a = node(8);
    let node_b = node(9);
    assert_eq!(manager.update_node_pn_info(&node_a, vec![]), (60_000, true));

    now.store(70_000, Ordering::SeqCst);
    assert_eq!(manager.update_node_pn_info(&node_b, vec![]), (60_000, true));

    now.store(60_000, Ordering::SeqCst);
    assert_eq!(
        manager.update_node_pn_info(&node_a, vec![assignment(1, None)]),
        (60_000, true)
    );
    assert_eq!(manager.get_node_pn_info(&node_b).unwrap().version, 60_000);
}
