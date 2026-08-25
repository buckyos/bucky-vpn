use crate::pn_server_info::{
    PnServerEndpoint, PnServerPortMapping, decode_pn_server_info,
};
use crate::server_config::*;
use p2p_frame::endpoint::{Endpoint as P2pEndpoint, Protocol};
use std::net::{IpAddr, SocketAddr};

fn config_from_yaml(yaml: &str) -> config::Config {
    config::Config::builder()
        .add_source(config::File::from_str(yaml, config::FileFormat::Yaml))
        .build()
        .unwrap()
}

fn pn_config(enabled: bool, transport: PnTransportMode) -> PnServerConfig {
    PnServerConfig {
        enabled,
        transport,
        control_server: None,
        report_interval_secs: 5,
        heartbeat_interval_secs: 5,
        heartbeat_timeout_secs: 15,
        advertised_ip: None,
        port_mapping: PnPortMappingConfig::default(),
        report_local_address: true,
    }
}

fn endpoint_protocols(endpoints: &[P2pEndpoint]) -> Vec<Protocol> {
    endpoints
        .iter()
        .map(|endpoint| endpoint.protocol())
        .collect()
}

#[test]
fn pn_transport_config_parses_exact_modes_and_control_candidate_order() {
    let expected = [
        ("tcp", PnTransportMode::Tcp, vec![Protocol::Tcp]),
        ("quic", PnTransportMode::Quic, vec![Protocol::Quic]),
        (
            "dual",
            PnTransportMode::Dual,
            vec![Protocol::Quic, Protocol::Tcp],
        ),
    ];

    for (value, mode, protocols) in expected {
        let config = config_from_yaml(&format!(
            r#"
pn:
  transport: {value}
  control_server:
    id: control-peer
    endpoint: "127.0.0.1:4624"
"#
        ));
        let pn = get_pn_server_config(&config).unwrap();

        assert_eq!(pn.transport, mode);
        let control = pn.control_server.unwrap();
        assert_eq!(endpoint_protocols(&control.endpoints), protocols);
        assert!(
            control
                .endpoints
                .iter()
                .all(|endpoint| endpoint.addr() == &"127.0.0.1:4624".parse().unwrap())
        );
    }
}

#[test]
fn pn_transport_config_omission_defaults_to_dual() {
    let config = config_from_yaml("pn:\n  enabled: true\n");
    let pn = get_pn_server_config(&config).unwrap();

    assert_eq!(pn.transport, PnTransportMode::Dual);
}

#[test]
fn pn_transport_config_example_matches_dual_runtime_contract_and_override_name() {
    let example = include_str!("../../config/config.example.yaml");
    let config = config_from_yaml(example);
    let sn = get_sn_server_config(&config);
    let pn = get_pn_server_config(&config).unwrap();

    assert!(example.contains("VPN_PN_TRANSPORT"));
    assert_eq!(pn.transport, PnTransportMode::Dual);
    assert!(validate_server_mode(&sn, &pn).is_ok());
}

#[test]
fn pn_transport_config_rejects_unknown_blank_case_variant_and_non_string_values() {
    for yaml in [
        "pn:\n  transport: udp\n",
        "pn:\n  transport: \"\"\n",
        "pn:\n  transport: TCP\n",
        "pn:\n  transport: 1\n",
        "pn:\n  transport: true\n",
    ] {
        let config = config_from_yaml(yaml);
        let err = get_pn_server_config(&config).unwrap_err().to_string();
        assert!(err.contains("pn.transport"), "unexpected error: {err}");
        assert!(err.contains("tcp"), "unexpected error: {err}");
        assert!(err.contains("quic"), "unexpected error: {err}");
        assert!(err.contains("dual"), "unexpected error: {err}");
    }
}

#[test]
fn pn_transport_config_modes_drive_service_primary_report_and_mapping() {
    let sn = SnServerConfig { enabled: false };
    let listen = P2pEndpoint::from((
        Protocol::Quic,
        "0.0.0.0:3624".parse::<SocketAddr>().unwrap(),
    ));
    let advertised_ip = "203.0.113.8".parse::<IpAddr>().unwrap();
    let mapping = PnPortMappingConfig {
        quic: Some(43624),
        tcp: Some(443),
    };
    let expected = [
        (
            PnTransportMode::Tcp,
            vec![Protocol::Tcp],
            Some(PnServerPortMapping {
                quic: None,
                tcp: Some(443),
            }),
        ),
        (
            PnTransportMode::Quic,
            vec![Protocol::Quic],
            Some(PnServerPortMapping {
                quic: Some(43624),
                tcp: None,
            }),
        ),
        (
            PnTransportMode::Dual,
            vec![Protocol::Quic, Protocol::Tcp],
            Some(PnServerPortMapping {
                quic: Some(43624),
                tcp: Some(443),
            }),
        ),
    ];

    for (mode, protocols, expected_mapping) in expected {
        let mut pn = pn_config(true, mode);
        pn.advertised_ip = Some(advertised_ip);
        pn.port_mapping = mapping.clone();
        let endpoints = resolve_service_endpoints(listen, &sn, &pn);
        let filtered_mapping = mode.filter_port_mapping(&pn.port_mapping);

        assert_eq!(endpoint_protocols(&endpoints), protocols);
        let report = endpoints_to_pn_server(
            "standalone-pn",
            endpoints.first().unwrap(),
            &endpoints,
            None,
            pn.advertised_ip,
            &filtered_mapping,
            true,
        );
        let payload = decode_pn_server_info(&report).unwrap();
        let reported_protocols: Vec<&str> = payload
            .endpoints
            .iter()
            .map(|endpoint| endpoint.protocol.as_str())
            .collect();
        let expected_reported: Vec<&str> = protocols
            .iter()
            .map(|protocol| match protocol {
                Protocol::Quic => PnServerEndpoint::PROTOCOL_QUIC,
                Protocol::Tcp => PnServerEndpoint::PROTOCOL_TCP,
                Protocol::Ext(_) => unreachable!(),
            })
            .collect();

        assert_eq!(reported_protocols, expected_reported);
        assert_eq!(
            payload.primary_endpoint().unwrap().protocol,
            expected_reported[0]
        );
        assert_eq!(payload.port_mapping, expected_mapping);
    }
}

#[test]
fn pn_transport_config_sn_only_keeps_dual_endpoints_and_combined_rejects_single_protocol() {
    let sn = SnServerConfig { enabled: true };
    let listen = P2pEndpoint::from((
        Protocol::Quic,
        "127.0.0.1:3624".parse::<SocketAddr>().unwrap(),
    ));

    for mode in [PnTransportMode::Tcp, PnTransportMode::Quic] {
        let disabled_pn = pn_config(false, mode);
        assert!(validate_server_mode(&sn, &disabled_pn).is_ok());
        assert_eq!(
            endpoint_protocols(&resolve_service_endpoints(listen, &sn, &disabled_pn)),
            vec![Protocol::Quic, Protocol::Tcp]
        );

        let combined = pn_config(true, mode);
        let err = validate_server_mode(&sn, &combined)
            .unwrap_err()
            .to_string();
        assert!(err.contains("pn.transport must be dual"));
    }

    let combined_dual = pn_config(true, PnTransportMode::Dual);
    assert!(validate_server_mode(&sn, &combined_dual).is_ok());
}
