use async_trait::async_trait;
use std::net::{IpAddr, Ipv4Addr};
use vpn_frame::client::{PacketRecv, VpnDevice};
use vpn_frame::errors::VpnResult;
use vpn_frame::server::NodeId;
use vpn_frame::{ClientProxyNodeInfo, NodeNetwork};

const VPN_DEVICE_SOURCE: &str = include_str!("../src/client/vpn_device.rs");
const VPN_CLIENT_SOURCE: &str = include_str!("../src/client/vpn_client.rs");

struct NoopRecv;

#[async_trait]
impl PacketRecv for NoopRecv {
    async fn on_recv(&self, _target: IpAddr, _packet: &[u8]) -> VpnResult<()> {
        Ok(())
    }
}

fn network() -> NodeNetwork {
    NodeNetwork {
        id: 10,
        group_id: 20,
        name: "network-before-refresh".to_string(),
        ip: Some(IpAddr::V4(Ipv4Addr::new(192, 168, 180, 150))),
        mask: 24,
        ipv6: None,
        ipv6_mask: 0,
        pn_server: None,
    }
}

fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function signature: {signature}"));
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing function body: {signature}"));
    let mut depth = 0usize;
    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function body: {signature}")
}

fn assert_ordered(source: &str, needles: &[&str]) {
    let mut offset = 0usize;
    for needle in needles {
        let relative = source[offset..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing ordered source fragment: {needle}"));
        offset += relative + needle.len();
    }
}

#[test]
fn public_update_device_remains_compatible_for_control_metadata_refresh() {
    let mut device = VpnDevice::<NoopRecv>::new(network());
    let mut refreshed = network();
    refreshed.group_id = 21;
    refreshed.name = "network-after-refresh".to_string();
    refreshed.pn_server = Some(ClientProxyNodeInfo {
        proxy_id: NodeId::from(vec![7u8; 32].as_slice()),
        name: Some("pn-after-restart".to_string()),
        endpoints: Vec::new(),
    });

    device
        .update_device(refreshed.clone())
        .expect("metadata-only refresh must keep the public update path compatible");

    assert!(device.network_info() == &refreshed);
}

#[test]
fn tun_effective_field_contract_excludes_control_and_dispatch_metadata() {
    let body = function_body(VPN_DEVICE_SOURCE, "fn tun_effective_changed");

    for field in ["id", "ip", "mask", "ipv6", "ipv6_mask"] {
        assert!(
            body.contains(&format!("current.{field} != desired.{field}")),
            "{field} must remain TUN-effective"
        );
    }
    for field in ["group_id", "name", "pn_server"] {
        assert!(
            !body.contains(&format!("current.{field}")),
            "{field} must not recreate the OS TUN"
        );
    }
}

#[test]
fn ipv6_tun_configuration_uses_the_ipv6_prefix() {
    let body = function_body(VPN_DEVICE_SOURCE, "pub fn create_device");

    assert!(body.contains(
        "config = config.ipv6(\n                self.network.ipv6.as_ref().unwrap().clone(),\n                self.network.ipv6_mask,"
    ));
    assert!(!body.contains(
        "config = config.ipv6(\n                self.network.ipv6.as_ref().unwrap().clone(),\n                self.network.mask,"
    ));
}

#[test]
fn dispatch_change_restarts_only_receive_context_and_missing_device_retries() {
    let body = function_body(VPN_DEVICE_SOURCE, "pub(crate) fn reconcile");

    assert!(body.contains("self.network.group_id != network.group_id"));
    assert_ordered(
        body,
        &[
            "self.network = network;",
            "self.recv = Some(recv);",
            "if tun_changed || self.dev.is_none()",
            "self.create_device()",
            "self.restart_recv_task();",
            "} else if dispatch_changed",
            "self.restart_recv_task();",
        ],
    );
    assert_eq!(
        body.matches("self.dev.take();").count(),
        1,
        "only the TUN-effective or missing-device branch may drop the handle"
    );
}

#[test]
fn failed_network_is_reinserted_before_error_and_stale_removal() {
    let body = function_body(VPN_CLIENT_SOURCE, "async fn run_proc");

    assert_ordered(
        body,
        &[
            "vpn_devices.remove(&network_id)",
            "vpn_device.reconcile",
            "vpn_devices.insert(network_id, vpn_device);",
            "if let Err(e) = device_result",
            "return Err(e);",
            "vpn_devices.retain",
            "*managed_devices = Some(vpn_devices);",
            "reconcile_result?;",
        ],
    );
}

#[test]
fn response_versions_commit_only_after_complete_reconciliation() {
    let body = function_body(VPN_CLIENT_SOURCE, "async fn run_proc");

    assert_ordered(
        body,
        &[
            "self.tunnel_factory.on_vpn_info_received(&vpn_infos).await?;",
            "let reconcile_result",
            "*managed_devices = Some(vpn_devices);",
            "reconcile_result?;",
            "self.cur_version.store(server_version",
            "self.cur_pn_info_version",
            ".store(pn_info_version",
            "self.is_first.store(false",
            "self.force_full_sync.store(false",
            "vpn info versions committed: info_version={}, pn_info_version={}",
        ],
    );
    assert_eq!(
        body.matches("vpn info versions committed:").count(),
        1,
        "the parseable version evidence must have one post-commit emission point"
    );
}

#[test]
fn zero_version_first_response_failure_keeps_the_next_request_full() {
    let constructor = function_body(VPN_CLIENT_SOURCE, "pub fn new_with_packet_dispatcher_config(");
    assert!(constructor.contains("cur_version: AtomicU16::new(0)"));
    assert!(constructor.contains("cur_pn_info_version: AtomicU32::new(0)"));
    assert!(constructor.contains("is_first: AtomicBool::new(true)"));
    assert!(constructor.contains("force_full_sync: AtomicBool::new(false)"));

    let body = function_body(VPN_CLIENT_SOURCE, "async fn run_proc");
    assert_ordered(
        body,
        &[
            "let is_first = self.is_first.load(Ordering::Relaxed)",
            ".get_vpn_info(None, None",
            "let reconcile_result",
            "reconcile_result?;",
            "self.cur_version.store(server_version",
            ".store(pn_info_version",
            "self.is_first.store(false",
        ],
    );
    assert_eq!(
        body.matches("self.is_first.store(false").count(),
        1,
        "the first-sync marker must have one successful commit point"
    );
}

#[test]
fn failed_same_second_incremental_apply_forces_a_full_retry() {
    let body = function_body(VPN_CLIENT_SOURCE, "async fn run_proc");

    assert!(body.contains("else if force_full_sync"));
    assert!(body.contains("get_vpn_info(None, None, None)"));
    assert_ordered(
        body,
        &[
            "if is_unchanged_vpn_info_response",
            "return Ok(());",
            "self.force_full_sync.store(true",
            "self.tunnel_factory.on_vpn_info_received(&vpn_infos).await?;",
            "reconcile_result?;",
            ".store(pn_info_version",
            "self.force_full_sync.store(false",
        ],
    );
}

#[test]
fn network_and_pn_version_atomic_widths_remain_intentionally_distinct() {
    assert!(VPN_CLIENT_SOURCE.contains("cur_version: AtomicU16"));
    assert!(VPN_CLIENT_SOURCE.contains("cur_pn_info_version: AtomicU32"));
    assert!(!VPN_CLIENT_SOURCE.contains("cur_version: AtomicU32"));
}
