mod pn_grouping {
    use super::*;
    use super::pn_registry::pn_registry_server;

    #[test]
    fn all_candidates_share_one_target_in_quic_first_advertised_order() {
        let v4 = IpAddr::from([192, 0, 2, 7]);
        let v6 = IpAddr::from([0x2001u16, 0xdb8, 0, 0, 0, 0, 0, 7]);
        let tcp_v4 = VpnEndpoint::new_tcp(v4, 443);
        let quic_v6 = VpnEndpoint::new(v6, 3624);
        let quic_v4 = VpnEndpoint::new(v4, 3625);
        let tcp_v6 = VpnEndpoint::new_tcp(v6, 8443);
        let mut pn = client_proxy(node_id(31), vec![
            tcp_v4.clone(), quic_v6.clone(), quic_v4.clone(),
            tcp_v6.clone(), quic_v6, tcp_v4,
        ]);
        pn.name = Some("pn.example".to_string());

        let target = P2pVpnTunnelFactory::pn_server_target(&pn).unwrap();

        assert_eq!(target.remote_ep, vec![
            Endpoint::from((Protocol::Quic, SocketAddr::new(v6, 3624))),
            Endpoint::from((Protocol::Quic, SocketAddr::new(v4, 3625))),
            Endpoint::from((Protocol::Tcp, SocketAddr::new(v4, 443))),
            Endpoint::from((Protocol::Tcp, SocketAddr::new(v6, 8443))),
        ]);
        assert_eq!(target.remote_id, P2pId::from(pn.proxy_id.as_slice()));
        assert_eq!(target.remote_name.as_deref(), Some("pn.example"));
        assert!(target.local_ep.is_none());
    }

    #[test]
    fn singleton_and_name_fallback_preserve_identity() {
        let endpoint = VpnEndpoint::new_tcp(IpAddr::from([127, 0, 0, 1]), 65535);
        for name in [None, Some(String::new()), Some(" ".to_string())] {
            let mut pn = client_proxy(node_id(32), vec![endpoint.clone()]);
            pn.name = name.clone();
            let target = P2pVpnTunnelFactory::pn_server_target(&pn).unwrap();
            assert_eq!(target.remote_ep, vec![Endpoint::from((
                Protocol::Tcp, SocketAddr::new(endpoint.ip, 65535),
            ))]);
            assert_eq!(target.remote_id, P2pId::from(pn.proxy_id.as_slice()));
            assert_eq!(target.remote_name, Some(
                name.filter(|value| !value.is_empty())
                    .unwrap_or_else(|| pn.proxy_id.to_p2p_base36())
            ));
        }
    }

    #[test]
    fn quic_alias_preserves_existing_advertised_deduplication() {
        let ip = IpAddr::from([127, 0, 0, 1]);
        let alias = VpnEndpoint::new_with_protocol("qic", ip, 3624);
        let canonical = VpnEndpoint::new(ip, 3624);
        let pn = client_proxy(node_id(33), vec![alias.clone(), canonical, alias]);
        let target = P2pVpnTunnelFactory::pn_server_target(&pn).unwrap();
        assert_eq!(target.remote_ep, vec![
            Endpoint::from((Protocol::Quic, SocketAddr::new(ip, 3624))); 2
        ]);
    }

    #[test]
    fn empty_candidates_fail_clearly() {
        let pn = client_proxy(node_id(34), vec![]);
        let err = P2pVpnTunnelFactory::pn_server_target(&pn).unwrap_err();
        assert_eq!(err.code(), VpnErrorCode::InvalidParam);
        assert!(err.msg().contains("no endpoint candidates"));
        assert!(err.msg().contains(&pn.proxy_id.to_base36()));
    }

    #[test]
    fn unsupported_candidate_fails_even_after_a_valid_candidate() {
        let pn = client_proxy(node_id(35), vec![
            VpnEndpoint::new(IpAddr::from([127, 0, 0, 1]), 3624),
            VpnEndpoint::new_with_protocol("http", IpAddr::from([127, 0, 0, 1]), 80),
        ]);
        let err = P2pVpnTunnelFactory::pn_server_target(&pn).unwrap_err();
        assert_eq!(err.code(), VpnErrorCode::InvalidParam);
        assert!(err.msg().contains("http"));
    }

    async fn factory() -> P2pVpnTunnelFactory {
        use p2p_frame::stack::{ServerRuntime, ServerRuntimeConfig};
        let identity: P2pIdentityRef = Arc::new(
            x509::generate_rsa_x509_identity(Some("pn-grouping-test".to_string())).unwrap()
        );
        let env = create_p2p_env(P2pConfig::new(
            Arc::new(X509IdentityFactory),
            Arc::new(X509IdentityCertFactory),
            vec![Endpoint::from((Protocol::Tcp, SocketAddr::from(([127, 0, 0, 1], 0))))],
            ServerRuntime::start(ServerRuntimeConfig::new().with_workers(1)).unwrap(),
        ).set_tcp_connect_timout(Duration::from_millis(100))).await.unwrap();
        let stack = create_p2p_stack(P2pStackConfig::new(env, identity)
            .set_conn_timeout(Duration::from_millis(100))).await.unwrap();
        P2pVpnTunnelFactory::new(stack, 3624, p2p_id(36),
            Arc::new(P2pVpnPnProxyRouteResolver::new()))
    }

    #[tokio::test]
    async fn dv_invalid_input_preserves_existing_registry() {
        let factory = factory().await;
        let retained = pn_registry_server(37);
        let target = P2pVpnTunnelFactory::pn_server_target(&retained).unwrap();
        factory.connected_pn_targets.lock().unwrap().insert(retained.clone(), target.clone());
        for endpoints in [vec![], vec![VpnEndpoint::new_with_protocol(
            "http", IpAddr::from([127, 0, 0, 1]), 80,
        )]] {
            let invalid = client_proxy(node_id(38), endpoints);
            let err = factory.sync_pn_server_connections(&[vpn_info(Some(invalid), vec![])])
                .await.unwrap_err();
            assert_eq!(err.code(), VpnErrorCode::InvalidParam);
            let connected = factory.connected_pn_targets.lock().unwrap();
            assert_eq!(connected.len(), 1);
            assert_eq!(connected[&retained].remote_ep, target.remote_ep);
        }
    }

    #[tokio::test]
    async fn dv_retention_removal_and_reappearance_use_full_targets() {
        let factory = factory().await;
        let retained = pn_registry_server(39);
        let target = P2pVpnTunnelFactory::pn_server_target(&retained).unwrap();
        factory.connected_pn_targets.lock().unwrap().insert(retained.clone(), target.clone());
        // A retained target must not redial these deliberately unserved addresses.
        factory.sync_pn_server_connections(&[
            vpn_info(Some(retained.clone()), vec![]),
            vpn_info(Some(retained.clone()), vec![]),
        ]).await.unwrap();
        assert_eq!(factory.connected_pn_targets.lock().unwrap()[&retained].remote_ep,
            target.remote_ep);
        factory.sync_pn_server_connections(&[]).await.unwrap();
        assert!(factory.connected_pn_targets.lock().unwrap().is_empty());

        let absent = client_proxy(node_id(40), vec![
            VpnEndpoint::new_tcp(IpAddr::from([127, 0, 0, 1]), 0),
            VpnEndpoint::new_tcp(IpAddr::from([127, 0, 0, 2]), 0),
        ]);
        let err = factory.sync_pn_server_connections(&[vpn_info(Some(absent.clone()), vec![])])
            .await.unwrap_err();
        assert_eq!(err.code(), VpnErrorCode::Failed);
        assert!(err.msg().contains("127.0.0.1:0"));
        assert!(err.msg().contains("127.0.0.2:0"));
        assert!(factory.connected_pn_targets.lock().unwrap().is_empty());

        // Seed one known retained entry to test identical metadata disappearing
        // and entering the real connection path when it reappears.
        let absent_target = P2pVpnTunnelFactory::pn_server_target(&absent).unwrap();
        factory.connected_pn_targets.lock().unwrap().insert(absent.clone(), absent_target);
        factory.sync_pn_server_connections(&[]).await.unwrap();
        let err = factory.sync_pn_server_connections(&[vpn_info(Some(absent), vec![])])
            .await.unwrap_err();
        assert_eq!(err.code(), VpnErrorCode::Failed);
    }
}
