use super::*;
use crate::pn_server_info::decode_pn_server_info;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

struct DelayedTunnelLookup {
    endpoint: Endpoint,
    available_after: usize,
    calls: AtomicUsize,
    requested_tunnels: Mutex<Vec<(String, TunnelId)>>,
}

impl DelayedTunnelLookup {
    fn new(endpoint: Endpoint, available_after: usize) -> Self {
        Self {
            endpoint,
            available_after,
            calls: AtomicUsize::new(0),
            requested_tunnels: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl ProxyControlTunnelLookup for DelayedTunnelLookup {
    async fn remote_endpoint(&self, peer_id: &PeerId, tunnel_id: TunnelId) -> Option<Endpoint> {
        self.requested_tunnels
            .lock()
            .unwrap()
            .push((peer_id.to_string(), tunnel_id));
        let attempt = self.calls.fetch_add(1, Ordering::SeqCst);
        (attempt >= self.available_after).then_some(self.endpoint)
    }
}

#[tokio::test]
async fn first_reonline_heartbeat_waits_for_exact_tunnel_registration() {
    let pn_node_id = NodeId::from(vec![7; 32].as_slice());
    let peer_id = PeerId::from(pn_node_id.as_slice());
    let tunnel_id = TunnelId::from(20260818);
    let endpoint = Endpoint::from((
        Protocol::Quic,
        "47.113.93.155:3625".parse().unwrap(),
    ));
    let lookup = DelayedTunnelLookup::new(endpoint, 2);

    let observed = observe_proxy_control_tunnel_with_retry(
        &lookup,
        &pn_node_id,
        tunnel_id,
        3,
        Duration::ZERO,
    )
    .await
    .unwrap()
    .unwrap();

    let payload = decode_pn_server_info(&observed).unwrap();
    assert_eq!(
        payload.endpoints[0].ip,
        "47.113.93.155".parse::<std::net::IpAddr>().unwrap()
    );
    assert_eq!(payload.endpoints[0].port, 3625);
    assert_eq!(lookup.calls.load(Ordering::SeqCst), 3);
    assert!(lookup
        .requested_tunnels
        .lock()
        .unwrap()
        .iter()
        .all(|request| request == &(peer_id.to_string(), tunnel_id)));
}

#[tokio::test]
async fn missing_exact_tunnel_uses_bounded_fallback() {
    let pn_node_id = NodeId::from(vec![8; 32].as_slice());
    let peer_id = PeerId::from(pn_node_id.as_slice());
    let tunnel_id = TunnelId::from(20260819);
    let endpoint = Endpoint::from((
        Protocol::Quic,
        "47.113.93.155:3625".parse().unwrap(),
    ));
    let lookup = DelayedTunnelLookup::new(endpoint, usize::MAX);

    let observed = observe_proxy_control_tunnel_with_retry(
        &lookup,
        &pn_node_id,
        tunnel_id,
        3,
        Duration::ZERO,
    )
    .await
    .unwrap();

    assert!(observed.is_none());
    assert_eq!(lookup.calls.load(Ordering::SeqCst), 3);
    assert!(lookup
        .requested_tunnels
        .lock()
        .unwrap()
        .iter()
        .all(|request| request == &(peer_id.to_string(), tunnel_id)));
}
