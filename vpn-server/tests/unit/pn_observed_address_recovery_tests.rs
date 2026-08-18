use super::*;

fn pn_server_with_payload(id: impl Into<String>, payload: PnServerInfoPayload) -> PnServerInfo {
    encode_pn_server_info(id, payload).unwrap()
}

fn payload(pn_server: &PnServerInfo) -> PnServerInfoPayload {
    decode_pn_server_info(pn_server).unwrap()
}

fn pn_identity(byte: u8) -> (NodeId, String) {
    let pn_node_id = NodeId::from(vec![byte; 32].as_slice());
    let pn_server_id = P2pId::from(pn_node_id.as_slice()).to_string();
    (pn_node_id, pn_server_id)
}

fn suppressed_address_report(id: &str) -> PnServerInfo {
    pn_server_with_payload(
        id,
        PnServerInfoPayload::new_with_primary_address(
            PnServerEndpoint::new_with_protocol(
                PnServerEndpoint::PROTOCOL_QUIC,
                "0.0.0.0".parse().unwrap(),
                3625,
            ),
            vec![PnServerEndpoint::new_tcp("0.0.0.0".parse().unwrap(), 3625)],
        )
        .with_port_mapping(Some(PnServerPortMapping {
            quic: Some(3625),
            tcp: Some(3625),
        })),
    )
}

fn observed_connection(id: &str, ip: &str, source_port: u16) -> PnServerInfo {
    pn_server_with_payload(
        id,
        PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(
            ip.parse().unwrap(),
            source_port,
        )),
    )
}

async fn heartbeat_with_observation(
    selector: &PnServerManager,
    pn_node_id: &NodeId,
    reported: &PnServerInfo,
    observation: Option<&PnServerInfo>,
) -> VpnResult<()> {
    selector
        .report_heartbeat_with_observation(
            pn_node_id,
            &ProxyNodeHeartbeat {
                heartbeat_id: vpn_frame::ProxyNodeHeartbeatId("recovery-test".to_string()),
                pn_server: Some(reported.clone()),
            },
            observation,
        )
        .await
}

fn selected_ip(selector_result: &PnServerInfo) -> IpAddr {
    payload(selector_result).endpoints[0].ip
}

#[tokio::test]
async fn reonline_heartbeat_refreshes_address_from_current_control_tunnel() {
    let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_millis(50));
    let (pn_node_id, pn_server_id) = pn_identity(7);
    let reported = suppressed_address_report(&pn_server_id);
    let first_observation = observed_connection(&pn_server_id, "47.113.93.155", 56000);
    let reonline_observation = observed_connection(&pn_server_id, "47.113.93.156", 57000);

    heartbeat_with_observation(&selector, &pn_node_id, &reported, Some(&first_observation))
        .await
        .unwrap();
    assert_eq!(
        selected_ip(&selector.select(1).await.unwrap().unwrap()),
        "47.113.93.155".parse::<IpAddr>().unwrap()
    );

    tokio::time::sleep(Duration::from_millis(100)).await;
    selector.prune_expired_remote_pn_servers_now();
    assert_eq!(selector.select(1).await.unwrap(), None);

    heartbeat_with_observation(
        &selector,
        &pn_node_id,
        &reported,
        Some(&reonline_observation),
    )
    .await
    .unwrap();
    assert_eq!(
        selected_ip(&selector.select(1).await.unwrap().unwrap()),
        "47.113.93.156".parse::<IpAddr>().unwrap()
    );
}

#[tokio::test]
async fn same_address_reonline_heartbeat_recovers_after_timeout() {
    let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_millis(50));
    let (pn_node_id, pn_server_id) = pn_identity(8);
    let reported = suppressed_address_report(&pn_server_id);
    let observed = observed_connection(&pn_server_id, "47.113.93.155", 56000);

    heartbeat_with_observation(&selector, &pn_node_id, &reported, Some(&observed))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    selector.prune_expired_remote_pn_servers_now();
    assert_eq!(selector.select(1).await.unwrap(), None);

    heartbeat_with_observation(&selector, &pn_node_id, &reported, Some(&observed))
        .await
        .unwrap();
    assert_eq!(
        selected_ip(&selector.select(1).await.unwrap().unwrap()),
        "47.113.93.155".parse::<IpAddr>().unwrap()
    );
}

#[tokio::test]
async fn unavailable_reonline_observation_keeps_last_valid_address() {
    let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_millis(50));
    let (pn_node_id, pn_server_id) = pn_identity(9);
    let reported = suppressed_address_report(&pn_server_id);
    let observed = observed_connection(&pn_server_id, "47.113.93.155", 56000);

    heartbeat_with_observation(&selector, &pn_node_id, &reported, Some(&observed))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    selector.prune_expired_remote_pn_servers_now();
    assert_eq!(selector.select(1).await.unwrap(), None);

    heartbeat_with_observation(&selector, &pn_node_id, &reported, None)
        .await
        .unwrap();
    assert_eq!(
        selected_ip(&selector.select(1).await.unwrap().unwrap()),
        "47.113.93.155".parse::<IpAddr>().unwrap()
    );
}

#[tokio::test]
async fn repeated_pruning_retains_observation_but_does_not_extend_liveness() {
    let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_millis(50));
    let (pn_node_id, pn_server_id) = pn_identity(10);
    let reported = suppressed_address_report(&pn_server_id);
    let observed = observed_connection(&pn_server_id, "47.113.93.155", 56000);

    heartbeat_with_observation(&selector, &pn_node_id, &reported, Some(&observed))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    selector.prune_expired_remote_pn_servers_now();
    selector.prune_expired_remote_pn_servers_now();

    let remote_pn_servers = selector.remote_pn_servers.lock().unwrap();
    let state = remote_pn_servers.get(&pn_server_id).unwrap();
    assert!(state.offline_logged);
    assert!(state.observed.is_some());
    assert!(!selector.remote_state_is_live(state, Instant::now()));
}

#[tokio::test]
async fn mismatched_observation_is_rejected_without_creating_state() {
    let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
    let (pn_node_id, pn_server_id) = pn_identity(11);
    let (_, other_pn_server_id) = pn_identity(12);
    let reported = suppressed_address_report(&pn_server_id);
    let other_observation = observed_connection(&other_pn_server_id, "47.113.93.155", 56000);

    let err =
        heartbeat_with_observation(&selector, &pn_node_id, &reported, Some(&other_observation))
            .await
            .unwrap_err();

    assert_eq!(err.code(), vpn_frame::errors::VpnErrorCode::InvalidParam);
    assert!(selector.remote_pn_servers.lock().unwrap().is_empty());
}
