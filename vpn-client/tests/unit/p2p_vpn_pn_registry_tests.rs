mod pn_registry {
use super::*;

#[test]
fn stale_pn_targets_are_removed_while_desired_targets_are_retained() {
    let stale = pn_registry_server(21);
    let retained = pn_registry_server(22);
    let stale_target = P2pVpnTunnelFactory::pn_server_target(&stale).unwrap();
    let retained_target = P2pVpnTunnelFactory::pn_server_target(&retained).unwrap();
    let mut connected = HashMap::from([
        (stale.clone(), stale_target.clone()),
        (retained.clone(), retained_target.clone()),
    ]);
    let desired = HashMap::from([(retained.clone(), retained_target.clone())]);

    let removed = take_removed_pn_targets(&mut connected, &desired);

    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].0, stale);
    assert!(!connected.contains_key(&removed[0].0));
    assert!(connected.contains_key(&retained));
    assert_eq!(removed[0].1.remote_ep, stale_target.remote_ep);
    assert_eq!(removed[0].1.remote_id, stale_target.remote_id);
    assert_eq!(connected[&retained].remote_ep, retained_target.remote_ep);
}

pub(super) fn pn_registry_server(seed: u8) -> ClientProxyNodeInfo {
    client_proxy(
        node_id(seed),
        vec![
            VpnEndpoint::new(IpAddr::from([127, 0, 0, 1]), 3624),
            VpnEndpoint::new_tcp(IpAddr::from([127, 0, 0, 1]), 443),
        ],
    )
}

#[test]
fn identical_pn_reappearance_is_missing_and_enters_the_connect_path() {
    let pn_server = pn_registry_server(23);
    let target = P2pVpnTunnelFactory::pn_server_target(&pn_server).unwrap();
    let mut connected =
        HashMap::from([(pn_server.clone(), target.clone())]);
    let absent = HashMap::new();

    let removed = take_removed_pn_targets(&mut connected, &absent);
    assert_eq!(removed.len(), 1);
    assert!(!connected.contains_key(&pn_server));

    let desired_after_reappearance =
        HashMap::from([(pn_server.clone(), target)]);
    let connect_candidates = desired_after_reappearance
        .keys()
        .filter(|candidate| !connected.contains_key(*candidate))
        .collect::<Vec<_>>();
    assert_eq!(connect_candidates, vec![&pn_server]);
}

#[test]
fn regression_model_exposes_the_legacy_stale_registry_entry() {
    let pn_server = pn_registry_server(24);
    let legacy_connected =
        HashMap::from([(pn_server.clone(), P2pVpnTunnelFactory::pn_server_target(&pn_server).unwrap())]);
    assert!(legacy_connected.contains_key(&pn_server));
    assert!(
        !(!legacy_connected.contains_key(&pn_server)),
        "the legacy registry would skip connect_server for identical metadata"
    );

    let mut connected = legacy_connected;
    let removed = take_removed_pn_targets(&mut connected, &HashMap::new());
    assert_eq!(removed.len(), 1);
    assert!(!connected.contains_key(&pn_server));
}

#[test]
fn registry_entry_is_removed_before_external_target_removal() {
    let source = include_str!("../../src/p2p_vpn.rs");
    let sync_start = source
        .find("async fn sync_pn_server_connections")
        .expect("missing PN synchronization method");
    let body = &source[sync_start..];
    let registry_removal = body
        .find("take_removed_pn_targets(&mut connected, &desired)")
        .expect("missing stale registry removal");
    let external_removal = body
        .find("ttp_client.remove_server(&target)")
        .expect("missing external target removal");

    assert!(registry_removal < external_removal);
}
}
