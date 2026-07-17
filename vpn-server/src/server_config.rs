use crate::pn_server_info::{
    PnServerEndpoint, PnServerInfoPayload, PnServerPortMapping, encode_pn_server_info,
};
use config::builder::DefaultState;
use if_addrs::IfAddr;
use p2p_frame::endpoint::{Endpoint as P2pEndpoint, Protocol};
use p2p_frame::p2p_identity::P2pId;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use vpn_frame::PnServerInfo;

const DEFAULT_YAML_CONFIG: &str = "config.yaml";
const LEGACY_TOML_CONFIG: &str = "config.toml";
const DEFAULT_NODE_TRAFFIC_IDEMPOTENCY_RETENTION_SECS: u64 = 600;

#[derive(Clone, Debug)]
pub struct SnServerConfig {
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnHttpConfig {
    pub ip: String,
    pub port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnAdminConfig {
    pub name: String,
    pub password: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnJwtConfig {
    pub key: String,
}

#[derive(Clone, Debug)]
pub struct PnServerConfig {
    pub enabled: bool,
    pub control_server: Option<PnControlServerConfig>,
    pub report_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub heartbeat_timeout_secs: u64,
    pub advertised_ip: Option<IpAddr>,
    pub port_mapping: PnPortMappingConfig,
    pub report_local_address: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PnTrafficUploadConfig {
    pub records_per_command: usize,
    pub concurrent_commands: usize,
    pub backlog_batches: usize,
    pub retry_delay_ms: u64,
    pub shutdown_drain_secs: u64,
}

impl Default for PnTrafficUploadConfig {
    fn default() -> Self {
        Self {
            records_per_command: 128,
            concurrent_commands: 4,
            backlog_batches: 64,
            retry_delay_ms: 250,
            shutdown_drain_secs: 5,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PnPortMappingConfig {
    pub quic: Option<u16>,
    pub tcp: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PnControlServerConfig {
    pub id: String,
    pub name: Option<String>,
    pub endpoint: P2pEndpoint,
}

pub fn select_default_config_file(current_dir: &Path) -> PathBuf {
    let yaml_config = current_dir.join(DEFAULT_YAML_CONFIG);
    if yaml_config.exists() {
        yaml_config
    } else {
        current_dir.join(LEGACY_TOML_CONFIG)
    }
}

pub fn build_server_config(
    explicit_config_file: Option<&str>,
    current_dir: &Path,
) -> Result<config::Config, config::ConfigError> {
    let config_file = explicit_config_file
        .map(PathBuf::from)
        .unwrap_or_else(|| select_default_config_file(current_dir));

    let config = config::ConfigBuilder::<DefaultState>::default()
        .set_default("ip", "0.0.0.0")?
        .set_default("port", 3624)?
        .set_default("http.ip", "0.0.0.0")?
        .set_default("http.port", 3445)?
        .set_default("sn.enabled", true)?
        .set_default("pn.enabled", true)?
        .set_default("pn.report_interval_secs", 5)?
        .set_default(
            "pn.node_traffic_idempotency_retention_secs",
            DEFAULT_NODE_TRAFFIC_IDEMPOTENCY_RETENTION_SECS as i64,
        )?
        .set_default("pn.heartbeat_interval_secs", 5)?
        .set_default("pn.heartbeat_timeout_secs", 15)?;

    let traffic_upload = PnTrafficUploadConfig::default();
    let mut config = config
        .set_default(
            "pn.traffic_upload.records_per_command",
            traffic_upload.records_per_command as i64,
        )?
        .set_default(
            "pn.traffic_upload.concurrent_commands",
            traffic_upload.concurrent_commands as i64,
        )?
        .set_default(
            "pn.traffic_upload.backlog_batches",
            traffic_upload.backlog_batches as i64,
        )?
        .set_default(
            "pn.traffic_upload.retry_delay_ms",
            traffic_upload.retry_delay_ms as i64,
        )?
        .set_default(
            "pn.traffic_upload.shutdown_drain_secs",
            traffic_upload.shutdown_drain_secs as i64,
        )?;

    if config_file.exists() {
        config = config.add_source(config::File::from(config_file.as_path()));
    }

    config = config.add_source(config::Environment::with_prefix("VPN").separator("_"));
    config.build()
}

pub fn get_pn_traffic_upload_config(
    config: &config::Config,
) -> Result<PnTrafficUploadConfig, config::ConfigError> {
    fn bounded_positive(
        config: &config::Config,
        key: &str,
        max: u64,
    ) -> Result<u64, config::ConfigError> {
        let value = config.get_int(key)?;
        if value <= 0 || value as u64 > max {
            return Err(config::ConfigError::Message(format!(
                "{key} must be within 1..={max}, got {value}"
            )));
        }
        Ok(value as u64)
    }

    Ok(PnTrafficUploadConfig {
        records_per_command: bounded_positive(
            config,
            "pn.traffic_upload.records_per_command",
            vpn_frame::MAX_TRAFFIC_RECORDS_PER_COMMAND as u64,
        )? as usize,
        concurrent_commands: bounded_positive(
            config,
            "pn.traffic_upload.concurrent_commands",
            64,
        )? as usize,
        backlog_batches: bounded_positive(
            config,
            "pn.traffic_upload.backlog_batches",
            4096,
        )? as usize,
        retry_delay_ms: bounded_positive(
            config,
            "pn.traffic_upload.retry_delay_ms",
            60_000,
        )?,
        shutdown_drain_secs: bounded_positive(
            config,
            "pn.traffic_upload.shutdown_drain_secs",
            300,
        )?,
    })
}

pub fn get_sn_server_config(config: &config::Config) -> SnServerConfig {
    SnServerConfig {
        enabled: config.get_bool("sn.enabled").unwrap_or(true),
    }
}

pub fn get_server_name_config(config: &config::Config) -> Option<String> {
    config.get_string("name").ok().and_then(|name| {
        let name = name.trim().to_owned();
        if name.is_empty() { None } else { Some(name) }
    })
}

pub fn get_sn_http_config(config: &config::Config) -> Result<SnHttpConfig, config::ConfigError> {
    let ip = get_string_prefer(config, "sn.http.ip", "http.ip")?;
    let port = get_int_prefer(config, "sn.http.port", "http.port")?;
    if port <= 0 || port > u16::MAX as i64 {
        return Err(config::ConfigError::Message(format!(
            "sn.http.port contains invalid port {port}"
        )));
    }
    Ok(SnHttpConfig {
        ip,
        port: port as u16,
    })
}

pub fn get_sn_admin_config(config: &config::Config) -> Result<SnAdminConfig, config::ConfigError> {
    Ok(SnAdminConfig {
        name: get_string_prefer(config, "sn.admin.name", "admin.name")?,
        password: get_string_prefer(config, "sn.admin.password", "admin.password")?,
    })
}

pub fn get_sn_jwt_config(config: &config::Config) -> Result<SnJwtConfig, config::ConfigError> {
    Ok(SnJwtConfig {
        key: get_string_prefer(config, "sn.jwt.key", "jwt.key")?,
    })
}

pub fn get_pn_server_config(
    config: &config::Config,
) -> Result<PnServerConfig, config::ConfigError> {
    let enabled = config.get_bool("pn.enabled").unwrap_or(true);

    let heartbeat_interval_secs = config
        .get_int("pn.heartbeat_interval_secs")
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value as u64)
        .unwrap_or(5);
    let heartbeat_timeout_secs = config
        .get_int("pn.heartbeat_timeout_secs")
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value as u64)
        .unwrap_or(15);
    if heartbeat_timeout_secs <= heartbeat_interval_secs {
        return Err(config::ConfigError::Message(format!(
            "pn.heartbeat_timeout_secs ({heartbeat_timeout_secs}) must be greater than pn.heartbeat_interval_secs ({heartbeat_interval_secs})"
        )));
    }

    Ok(PnServerConfig {
        enabled,
        control_server: get_pn_control_server_config(config)?,
        report_interval_secs: config
            .get_int("pn.report_interval_secs")
            .ok()
            .filter(|value| *value > 0)
            .map(|value| value as u64)
            .unwrap_or(5),
        heartbeat_interval_secs,
        heartbeat_timeout_secs,
        advertised_ip: get_pn_advertised_ip_config(config)?,
        port_mapping: get_pn_port_mapping_config(config)?,
        report_local_address: config.get_bool("pn.report_local_address").unwrap_or(true),
    })
}

pub fn get_node_traffic_idempotency_retention_secs(
    config: &config::Config,
) -> Result<u64, config::ConfigError> {
    let key = "pn.node_traffic_idempotency_retention_secs";
    let value = config.get::<config::Value>(key)?;
    let parsed = match value.kind {
        config::ValueKind::I64(value) => u64::try_from(value).ok(),
        config::ValueKind::I128(value) => u64::try_from(value).ok(),
        config::ValueKind::U64(value) => Some(value),
        config::ValueKind::U128(value) => u64::try_from(value).ok(),
        config::ValueKind::String(value) => value.parse::<u64>().ok(),
        _ => None,
    };

    match parsed {
        Some(value) if value > 0 => Ok(value),
        _ => Err(config::ConfigError::Message(format!(
            "{key} must be a positive integer that fits in u64"
        ))),
    }
}

pub fn should_start_pn_server(sn_config: &SnServerConfig, pn_config: &PnServerConfig) -> bool {
    let _ = sn_config;
    pn_config.enabled
}

pub fn is_standalone_proxy_node(sn_config: &SnServerConfig, pn_config: &PnServerConfig) -> bool {
    !sn_config.enabled && should_start_pn_server(sn_config, pn_config)
}

pub fn validate_server_mode(
    sn_config: &SnServerConfig,
    pn_config: &PnServerConfig,
) -> Result<(), config::ConfigError> {
    if !pn_config.enabled {
        return Ok(());
    }
    if sn_config.enabled {
        if pn_config.control_server.is_some() {
            return Err(config::ConfigError::Message(
                "pn.control_server must not be configured when the control plane and proxy run in the same process"
                    .to_owned(),
            ));
        }
        return Ok(());
    }

    let control_server = pn_config.control_server.as_ref().ok_or_else(|| {
        config::ConfigError::Message(
            "standalone proxy mode requires pn.control_server".to_owned(),
        )
    })?;
    P2pId::from_str(control_server.id.as_str()).map_err(|err| {
        config::ConfigError::Message(format!(
            "pn.control_server.id contains an invalid P2P identity: {err}"
        ))
    })?;
    Ok(())
}

pub fn resolve_service_endpoints(
    sn_endpoint: P2pEndpoint,
    _sn_config: &SnServerConfig,
    _pn_config: &PnServerConfig,
) -> Vec<P2pEndpoint> {
    let tcp_endpoint = P2pEndpoint::from((Protocol::Tcp, *sn_endpoint.addr()));
    if sn_endpoint.protocol() == Protocol::Tcp {
        vec![sn_endpoint]
    } else {
        vec![sn_endpoint, tcp_endpoint]
    }
}

pub fn endpoint_to_pn_server(
    id: &str,
    endpoint: &P2pEndpoint,
) -> PnServerInfo {
    let addr = endpoint.addr();
    encode_pn_server_info(
        id.to_string(),
        PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new_with_protocol(
            pn_endpoint_protocol(endpoint.protocol()),
            addr.ip(),
            addr.port(),
        )),
    )
    .unwrap()
}

pub fn endpoints_to_pn_server(
    id: &str,
    primary_endpoint: &P2pEndpoint,
    endpoints: &[P2pEndpoint],
    route_hint: Option<&P2pEndpoint>,
    advertised_ip: Option<IpAddr>,
    port_mapping: &PnPortMappingConfig,
    report_local_address: bool,
) -> PnServerInfo {
    let primary = p2p_endpoint_to_reported_pn_endpoint(
        primary_endpoint,
        route_hint,
        advertised_ip,
        report_local_address,
    );
    let addresses = endpoints
        .iter()
        .map(|endpoint| {
            p2p_endpoint_to_reported_pn_endpoint(
                endpoint,
                route_hint,
                advertised_ip,
                report_local_address,
            )
        })
        .collect();
    encode_pn_server_info(
        id.to_string(),
        PnServerInfoPayload::new_with_primary_address(primary, addresses)
            .with_advertised_ip(advertised_ip)
            .with_port_mapping(pn_server_port_mapping(port_mapping)),
    )
    .unwrap()
}

fn p2p_endpoint_to_reported_pn_endpoint(
    endpoint: &P2pEndpoint,
    route_hint: Option<&P2pEndpoint>,
    configured_advertised_ip: Option<IpAddr>,
    report_local_address: bool,
) -> PnServerEndpoint {
    if let Some(advertised_ip) = configured_advertised_ip {
        let addr = endpoint.addr();
        PnServerEndpoint::new_with_protocol(
            pn_endpoint_protocol(endpoint.protocol()),
            advertised_ip,
            addr.port(),
        )
    } else if report_local_address {
        let addr = endpoint.addr();
        PnServerEndpoint::new_with_protocol(
            pn_endpoint_protocol(endpoint.protocol()),
            advertised_ip_for(addr.ip(), route_hint),
            addr.port(),
        )
    } else {
        let addr = endpoint.addr();
        PnServerEndpoint::new_with_protocol(
            pn_endpoint_protocol(endpoint.protocol()),
            unspecified_ip_for(addr.ip()),
            addr.port(),
        )
    }
}

pub fn p2p_endpoint_to_pn_endpoint(
    endpoint: &P2pEndpoint,
) -> PnServerEndpoint {
    let addr = endpoint.addr();
    PnServerEndpoint::new_with_protocol(
        pn_endpoint_protocol(endpoint.protocol()),
        addr.ip(),
        addr.port(),
    )
}

fn unspecified_ip_for(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    }
}

fn pn_server_port_mapping(port_mapping: &PnPortMappingConfig) -> Option<PnServerPortMapping> {
    let mapping = PnServerPortMapping {
        quic: port_mapping.quic,
        tcp: port_mapping.tcp,
    };
    (!mapping.is_empty()).then_some(mapping)
}

fn pn_endpoint_protocol(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Quic => PnServerEndpoint::PROTOCOL_QUIC,
        Protocol::Tcp => PnServerEndpoint::PROTOCOL_TCP,
        Protocol::Ext(_) => PnServerEndpoint::PROTOCOL_QUIC,
    }
}

fn advertised_ip_for(listen_ip: IpAddr, route_hint: Option<&P2pEndpoint>) -> IpAddr {
    if !listen_ip.is_unspecified() {
        return listen_ip;
    }

    route_hint
        .and_then(|endpoint| route_local_ip(listen_ip, *endpoint.addr()))
        .or_else(|| first_non_loopback_interface_ip(listen_ip))
        .unwrap_or(listen_ip)
}

fn route_local_ip(listen_ip: IpAddr, remote_addr: SocketAddr) -> Option<IpAddr> {
    if !same_ip_family(listen_ip, remote_addr.ip()) {
        return None;
    }

    let socket = UdpSocket::bind(SocketAddr::new(listen_ip, 0)).ok()?;
    socket.connect(remote_addr).ok()?;
    let local_ip = socket.local_addr().ok()?.ip();
    (!local_ip.is_unspecified()).then_some(local_ip)
}

fn first_non_loopback_interface_ip(listen_ip: IpAddr) -> Option<IpAddr> {
    if_addrs::get_if_addrs()
        .ok()?
        .into_iter()
        .find_map(|iface| {
            if iface.is_loopback() {
                return None;
            }
            let ip = match iface.addr {
                IfAddr::V4(addr) => IpAddr::V4(addr.ip),
                IfAddr::V6(addr) => IpAddr::V6(addr.ip),
            };
            same_ip_family(listen_ip, ip).then_some(ip)
        })
}

fn same_ip_family(left: IpAddr, right: IpAddr) -> bool {
    matches!(
        (left, right),
        (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_))
    )
}

fn get_pn_control_server_config(
    config: &config::Config,
) -> Result<Option<PnControlServerConfig>, config::ConfigError> {
    get_control_server_config_at(config, "pn.control_server")
}

fn get_pn_port_mapping_config(
    config: &config::Config,
) -> Result<PnPortMappingConfig, config::ConfigError> {
    Ok(PnPortMappingConfig {
        quic: get_optional_port_prefer(config, "pn.port_mapping.quic", "pn.map_ports.quic")?,
        tcp: get_optional_port_prefer(config, "pn.port_mapping.tcp", "pn.map_ports.tcp")?,
    })
}

fn get_pn_advertised_ip_config(
    config: &config::Config,
) -> Result<Option<IpAddr>, config::ConfigError> {
    let Some(value) = get_optional_trimmed_string(config, "pn.advertised_ip")? else {
        return Ok(None);
    };
    let ip = value.parse::<IpAddr>().map_err(|err| {
        config::ConfigError::Message(format!(
            "pn.advertised_ip contains invalid IP address {value:?}: {err}"
        ))
    })?;
    if ip.is_unspecified() {
        return Err(config::ConfigError::Message(format!(
            "pn.advertised_ip must be a concrete IP address, got {ip}"
        )));
    }
    Ok(Some(ip))
}

fn get_control_server_config_at(
    config: &config::Config,
    prefix: &str,
) -> Result<Option<PnControlServerConfig>, config::ConfigError> {
    let id_key = format!("{prefix}.id");
    let name_key = format!("{prefix}.name");
    let endpoint_key = format!("{prefix}.endpoint");
    let id = match config.get_string(id_key.as_str()) {
        Ok(id) => id,
        Err(config::ConfigError::NotFound(_)) => {
            if config.get_string(endpoint_key.as_str()).is_ok()
                || config.get_string(name_key.as_str()).is_ok()
            {
                return Err(config::ConfigError::Message(format!(
                    "{prefix}.id is required when {prefix}.endpoint or {prefix}.name is configured"
                )));
            }
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    let endpoint = config.get_string(endpoint_key.as_str())?;
    Ok(Some(PnControlServerConfig {
        id,
        name: get_optional_trimmed_string(config, name_key.as_str())?,
        endpoint: parse_quic_endpoint(&endpoint)?,
    }))
}

fn get_optional_trimmed_string(
    config: &config::Config,
    key: &str,
) -> Result<Option<String>, config::ConfigError> {
    match config.get_string(key) {
        Ok(value) => {
            let value = value.trim().to_owned();
            Ok(if value.is_empty() { None } else { Some(value) })
        }
        Err(config::ConfigError::NotFound(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

fn get_string_prefer(
    config: &config::Config,
    preferred: &str,
    legacy: &str,
) -> Result<String, config::ConfigError> {
    match config.get_string(preferred) {
        Ok(value) => Ok(value),
        Err(config::ConfigError::NotFound(_)) => config.get_string(legacy),
        Err(err) => Err(err),
    }
}

fn get_int_prefer(
    config: &config::Config,
    preferred: &str,
    legacy: &str,
) -> Result<i64, config::ConfigError> {
    match config.get_int(preferred) {
        Ok(value) => Ok(value),
        Err(config::ConfigError::NotFound(_)) => config.get_int(legacy),
        Err(err) => Err(err),
    }
}

fn get_optional_port_prefer(
    config: &config::Config,
    preferred: &str,
    legacy: &str,
) -> Result<Option<u16>, config::ConfigError> {
    let value = match config.get_int(preferred) {
        Ok(value) => Some(value),
        Err(config::ConfigError::NotFound(_)) => match config.get_int(legacy) {
            Ok(value) => Some(value),
            Err(config::ConfigError::NotFound(_)) => None,
            Err(err) => return Err(err),
        },
        Err(err) => return Err(err),
    };
    value
        .map(|value| {
            if value <= 0 || value > u16::MAX as i64 {
                return Err(config::ConfigError::Message(format!(
                    "{preferred} contains invalid port {value}"
                )));
            }
            Ok(value as u16)
        })
        .transpose()
}

fn parse_quic_endpoint(address: &str) -> Result<P2pEndpoint, config::ConfigError> {
    let socket_addr = address.parse::<SocketAddr>().map_err(|err| {
        config::ConfigError::Message(format!(
            "pn control server endpoint contains invalid socket address {address:?}: {err}"
        ))
    })?;
    Ok(P2pEndpoint::from((Protocol::Quic, socket_addr)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    fn new_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bucky-vpn-server-config-{}-{}-{}",
            std::process::id(),
            TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_config_prefers_yaml_over_legacy_toml() {
        let dir = new_temp_dir();
        fs::write(dir.join(DEFAULT_YAML_CONFIG), "port: 1111\n").unwrap();
        fs::write(dir.join(LEGACY_TOML_CONFIG), "port = 2222\n").unwrap();

        let selected = select_default_config_file(&dir);
        assert_eq!(selected, dir.join(DEFAULT_YAML_CONFIG));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn default_config_falls_back_to_legacy_toml_when_yaml_missing() {
        let dir = new_temp_dir();
        fs::write(dir.join(LEGACY_TOML_CONFIG), "port = 2222\n").unwrap();

        let selected = select_default_config_file(&dir);
        assert_eq!(selected, dir.join(LEGACY_TOML_CONFIG));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sn_and_pn_default_enabled_without_config_file() {
        let dir = new_temp_dir();
        let config = build_server_config(None, &dir).unwrap();
        let sn_config = get_sn_server_config(&config);
        let pn_config = get_pn_server_config(&config).unwrap();

        assert!(sn_config.enabled);
        assert!(pn_config.enabled);
        assert_eq!(pn_config.report_interval_secs, 5);
        assert_eq!(pn_config.heartbeat_interval_secs, 5);
        assert_eq!(pn_config.heartbeat_timeout_secs, 15);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_can_enable_pn_server() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
pn:
  enabled: true
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let pn_config = get_pn_server_config(&config).unwrap();

        assert!(pn_config.enabled);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_can_disable_default_sn_and_pn_servers() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
sn:
  enabled: false
pn:
  enabled: false
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let sn_config = get_sn_server_config(&config);
        let pn_config = get_pn_server_config(&config).unwrap();

        assert!(!sn_config.enabled);
        assert!(!pn_config.enabled);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_can_configure_server_name() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
name: " proxy-a "
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();

        assert_eq!(get_server_name_config(&config).as_deref(), Some("proxy-a"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_can_configure_sn_owned_management_config() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
sn:
  http:
    ip: "127.0.0.1"
    port: 8080
  admin:
    name: "owner"
    password: "secret"
  jwt:
    key: "sn-jwt-secret"
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let http_config = get_sn_http_config(&config).unwrap();
        let admin_config = get_sn_admin_config(&config).unwrap();
        let jwt_config = get_sn_jwt_config(&config).unwrap();

        assert_eq!(
            http_config,
            SnHttpConfig {
                ip: "127.0.0.1".to_string(),
                port: 8080,
            }
        );
        assert_eq!(
            admin_config,
            SnAdminConfig {
                name: "owner".to_string(),
                password: "secret".to_string(),
            }
        );
        assert_eq!(
            jwt_config,
            SnJwtConfig {
                key: "sn-jwt-secret".to_string(),
            }
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_top_level_management_config_remains_compatible() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
http:
  ip: "127.0.0.2"
  port: 9090
admin:
  name: "legacy"
  password: "legacy-secret"
jwt:
  key: "legacy-jwt-secret"
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let http_config = get_sn_http_config(&config).unwrap();
        let admin_config = get_sn_admin_config(&config).unwrap();
        let jwt_config = get_sn_jwt_config(&config).unwrap();

        assert_eq!(
            http_config,
            SnHttpConfig {
                ip: "127.0.0.2".to_string(),
                port: 9090,
            }
        );
        assert_eq!(
            admin_config,
            SnAdminConfig {
                name: "legacy".to_string(),
                password: "legacy-secret".to_string(),
            }
        );
        assert_eq!(
            jwt_config,
            SnJwtConfig {
                key: "legacy-jwt-secret".to_string(),
            }
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pn_server_start_depends_on_pn_enabled_only() {
        let enabled_sn = SnServerConfig { enabled: true };
        let disabled_sn = SnServerConfig { enabled: false };
        let enabled_pn = PnServerConfig {
            enabled: true,
            control_server: None,
            report_interval_secs: 5,
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            advertised_ip: None,
            port_mapping: PnPortMappingConfig::default(),
            report_local_address: true,
        };
        let disabled_pn = PnServerConfig {
            enabled: false,
            control_server: None,
            report_interval_secs: 5,
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            advertised_ip: None,
            port_mapping: PnPortMappingConfig::default(),
            report_local_address: true,
        };

        assert!(should_start_pn_server(&enabled_sn, &enabled_pn));
        assert!(should_start_pn_server(&disabled_sn, &enabled_pn));
        assert!(!should_start_pn_server(&enabled_sn, &disabled_pn));
        assert!(!should_start_pn_server(&disabled_sn, &disabled_pn));
    }

    #[test]
    fn standalone_proxy_node_requires_disabled_sn_and_enabled_pn() {
        let enabled_sn = SnServerConfig { enabled: true };
        let disabled_sn = SnServerConfig { enabled: false };
        let enabled_pn = PnServerConfig {
            enabled: true,
            control_server: None,
            report_interval_secs: 5,
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            advertised_ip: None,
            port_mapping: PnPortMappingConfig::default(),
            report_local_address: true,
        };
        let disabled_pn = PnServerConfig {
            enabled: false,
            control_server: None,
            report_interval_secs: 5,
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            advertised_ip: None,
            port_mapping: PnPortMappingConfig::default(),
            report_local_address: true,
        };

        assert!(is_standalone_proxy_node(&disabled_sn, &enabled_pn));
        assert!(!is_standalone_proxy_node(&enabled_sn, &enabled_pn));
        assert!(!is_standalone_proxy_node(&disabled_sn, &disabled_pn));
    }

    #[test]
    fn proxy_address_defaults_to_control_endpoint_without_static_proxy_addresses() {
        let sn = SnServerConfig { enabled: true };
        let pn = PnServerConfig {
            enabled: true,
            control_server: None,
            report_interval_secs: 5,
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            advertised_ip: None,
            port_mapping: PnPortMappingConfig::default(),
            report_local_address: true,
        };
        let sn_endpoint = parse_quic_endpoint("127.0.0.1:3624").unwrap();

        let endpoints = resolve_service_endpoints(sn_endpoint.clone(), &sn, &pn);

        assert_eq!(endpoints.len(), 2);
        assert_eq!(format!("{:?}", endpoints[0]), format!("{:?}", sn_endpoint));
        assert_eq!(endpoints[1].protocol(), Protocol::Tcp);
        assert_eq!(endpoints[1].addr(), sn_endpoint.addr());
    }

    #[test]
    fn service_endpoints_do_not_include_static_proxy_addresses() {
        let sn = SnServerConfig { enabled: true };
        let disabled_sn = SnServerConfig { enabled: false };
        let pn = PnServerConfig {
            enabled: true,
            control_server: None,
            report_interval_secs: 5,
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            advertised_ip: None,
            port_mapping: PnPortMappingConfig::default(),
            report_local_address: true,
        };
        let sn_endpoint = parse_quic_endpoint("127.0.0.1:3624").unwrap();

        let endpoints = resolve_service_endpoints(sn_endpoint.clone(), &sn, &pn);
        assert_eq!(endpoints.len(), 2);

        let endpoints = resolve_service_endpoints(sn_endpoint, &disabled_sn, &pn);
        assert_eq!(endpoints.len(), 2);
    }

    #[test]
    fn endpoint_to_pn_server_preserves_unspecified_ip() {
        let endpoint = parse_quic_endpoint("0.0.0.0:4600").unwrap();

        let pn_server = endpoint_to_pn_server("remote-node-id", &endpoint);

        let payload = crate::pn_server_info::decode_pn_server_info(&pn_server).unwrap();
        assert_eq!(
            payload.endpoints,
            vec![PnServerEndpoint::new(
                "0.0.0.0".parse::<IpAddr>().unwrap(),
                4600
            )]
        );
    }

    #[test]
    fn endpoints_to_pn_server_reports_advertised_ip_and_port_mapping() {
        let quic_endpoint = parse_quic_endpoint("127.0.0.1:3624").unwrap();
        let tcp_endpoint = P2pEndpoint::from((Protocol::Tcp, *quic_endpoint.addr()));
        let advertised_ip = "203.0.113.7".parse::<IpAddr>().unwrap();
        let port_mapping = PnPortMappingConfig {
            quic: Some(43624),
            tcp: Some(443),
        };

        let pn_server = endpoints_to_pn_server(
            "remote-node-id",
            &quic_endpoint,
            &[quic_endpoint, tcp_endpoint],
            None,
            Some(advertised_ip),
            &port_mapping,
            true,
        );

        let payload = crate::pn_server_info::decode_pn_server_info(&pn_server).unwrap();
        assert_eq!(
            payload.endpoints,
            vec![
                PnServerEndpoint::new_with_protocol(
                    PnServerEndpoint::PROTOCOL_QUIC,
                    advertised_ip,
                    3624,
                ),
                PnServerEndpoint::new_tcp(advertised_ip, 3624),
            ]
        );
        assert_eq!(payload.advertised_ip, Some(advertised_ip));
        assert_eq!(
            payload.port_mapping,
            Some(PnServerPortMapping {
                quic: Some(43624),
                tcp: Some(443),
            })
        );

        let pn_server_without_mapping = endpoints_to_pn_server(
            "remote-node-id",
            &quic_endpoint,
            &[quic_endpoint, tcp_endpoint],
            None,
            Some(advertised_ip),
            &PnPortMappingConfig::default(),
            true,
        );
        let payload_without_mapping =
            crate::pn_server_info::decode_pn_server_info(&pn_server_without_mapping).unwrap();
        assert_eq!(payload_without_mapping.port_mapping, None);
        assert!(
            payload_without_mapping
                .endpoints
                .iter()
                .all(|endpoint| endpoint.port == 3624)
        );
    }

    #[test]
    fn advertised_ip_overrides_discovery_and_report_local_false() {
        let quic_endpoint = parse_quic_endpoint("0.0.0.0:3624").unwrap();
        let tcp_endpoint = P2pEndpoint::from((Protocol::Tcp, *quic_endpoint.addr()));
        let route_hint = parse_quic_endpoint("127.0.0.1:4600").unwrap();
        let advertised_ip = "198.51.100.9".parse::<IpAddr>().unwrap();
        let port_mapping = PnPortMappingConfig {
            quic: Some(43624),
            tcp: Some(443),
        };

        let pn_server = endpoints_to_pn_server(
            "remote-node-id",
            &quic_endpoint,
            &[quic_endpoint, tcp_endpoint],
            Some(&route_hint),
            Some(advertised_ip),
            &port_mapping,
            false,
        );

        let payload = crate::pn_server_info::decode_pn_server_info(&pn_server).unwrap();
        assert_eq!(
            payload.endpoints,
            vec![
                PnServerEndpoint::new_with_protocol(
                    PnServerEndpoint::PROTOCOL_QUIC,
                    advertised_ip,
                    3624,
                ),
                PnServerEndpoint::new_tcp(advertised_ip, 3624),
            ]
        );
        assert_eq!(payload.advertised_ip, Some(advertised_ip));
        assert_eq!(
            payload.port_mapping,
            Some(PnServerPortMapping {
                quic: Some(43624),
                tcp: Some(443),
            })
        );
    }

    #[test]
    fn yaml_can_configure_pn_control_server() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
pn:
  control_server:
    id: "server-peer"
    endpoint: "127.0.0.1:3624"
  report_interval_secs: 9
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let pn_config = get_pn_server_config(&config).unwrap();

        assert_eq!(
            pn_config.control_server,
            Some(PnControlServerConfig {
                id: "server-peer".to_string(),
                name: None,
                endpoint: parse_quic_endpoint("127.0.0.1:3624").unwrap(),
            })
        );
        assert_eq!(pn_config.report_interval_secs, 9);
        assert_eq!(pn_config.heartbeat_interval_secs, 5);
        assert_eq!(pn_config.heartbeat_timeout_secs, 15);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_can_configure_pn_advertised_ip_ipv4_ipv6_and_port_mapping() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
pn:
  advertised_ip: "203.0.113.7"
  port_mapping:
    quic: 43624
    tcp: 443
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let pn_config = get_pn_server_config(&config).unwrap();

        assert_eq!(
            pn_config.advertised_ip,
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            pn_config.port_mapping,
            PnPortMappingConfig {
                quic: Some(43624),
                tcp: Some(443),
            }
        );
        assert!(pn_config.report_local_address);

        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
pn:
  advertised_ip: "2001:db8::7"
  report_local_address: false
"#,
        )
        .unwrap();
        let config = build_server_config(None, &dir).unwrap();
        let pn_config = get_pn_server_config(&config).unwrap();
        assert_eq!(
            pn_config.advertised_ip,
            Some("2001:db8::7".parse().unwrap())
        );
        assert!(!pn_config.report_local_address);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_rejects_invalid_or_unspecified_pn_advertised_ip() {
        let dir = new_temp_dir();
        for advertised_ip in ["not-an-ip", "0.0.0.0", "::"] {
            fs::write(
                dir.join(DEFAULT_YAML_CONFIG),
                format!("pn:\n  advertised_ip: \"{advertised_ip}\"\n"),
            )
            .unwrap();

            let config = build_server_config(None, &dir).unwrap();
            assert!(
                get_pn_server_config(&config).is_err(),
                "advertised_ip {advertised_ip:?} must be rejected"
            );
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sn_control_server_is_not_pn_control_config() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
sn:
  control_server:
    id: "sn-server-peer"
    endpoint: "127.0.0.1:4624"
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let pn_config = get_pn_server_config(&config).unwrap();

        assert_eq!(pn_config.control_server, None);

        let _ = fs::remove_dir_all(dir);
    }
}
