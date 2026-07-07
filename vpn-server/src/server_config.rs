use crate::sqlite_store_factory::{ProxyNodeApproval, ProxyNodeApprovalStatus, SqliteStoreFactory};
use config::builder::DefaultState;
use if_addrs::IfAddr;
use p2p_frame::endpoint::{Endpoint as P2pEndpoint, Protocol};
use p2p_frame::p2p_identity::P2pId;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vpn_frame::errors::VpnResult;
use vpn_frame::server::{NetworkId, NodeId, PnServerSelector, VpnStoreFactory};
use vpn_frame::{Endpoint, PnServerInfo, PnServerPortMapping};

const DEFAULT_YAML_CONFIG: &str = "config.yaml";
const LEGACY_TOML_CONFIG: &str = "config.toml";

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
    pub port_mapping: PnPortMappingConfig,
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

    let mut config = config::ConfigBuilder::<DefaultState>::default()
        .set_default("ip", "0.0.0.0")?
        .set_default("port", 3624)?
        .set_default("http.ip", "0.0.0.0")?
        .set_default("http.port", 3445)?
        .set_default("sn.enabled", true)?
        .set_default("pn.enabled", true)?
        .set_default("pn.report_interval_secs", 5)?;

    if config_file.exists() {
        config = config.add_source(config::File::from(config_file.as_path()));
    }

    config = config.add_source(config::Environment::with_prefix("VPN").separator("_"));
    config.build()
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

    Ok(PnServerConfig {
        enabled,
        control_server: get_pn_control_server_config(config)?,
        report_interval_secs: config
            .get_int("pn.report_interval_secs")
            .ok()
            .filter(|value| *value > 0)
            .map(|value| value as u64)
            .unwrap_or(5),
        port_mapping: get_pn_port_mapping_config(config)?,
    })
}

pub fn should_start_pn_server(sn_config: &SnServerConfig, pn_config: &PnServerConfig) -> bool {
    let _ = sn_config;
    pn_config.enabled
}

pub fn is_standalone_proxy_node(sn_config: &SnServerConfig, pn_config: &PnServerConfig) -> bool {
    !sn_config.enabled && should_start_pn_server(sn_config, pn_config)
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
    route_hint: Option<&P2pEndpoint>,
) -> PnServerInfo {
    let addr = endpoint.addr();
    PnServerInfo::new_with_endpoint(
        id.to_string(),
        Endpoint::new_with_protocol(
            pn_endpoint_protocol(endpoint.protocol()),
            advertised_ip_for(addr.ip(), route_hint),
            addr.port(),
        ),
    )
}

pub fn endpoints_to_pn_server(
    id: &str,
    primary_endpoint: &P2pEndpoint,
    endpoints: &[P2pEndpoint],
    route_hint: Option<&P2pEndpoint>,
    port_mapping: &PnPortMappingConfig,
) -> PnServerInfo {
    let primary = p2p_endpoint_to_pn_endpoint(primary_endpoint, route_hint);
    let addresses = endpoints
        .iter()
        .map(|endpoint| p2p_endpoint_to_pn_endpoint(endpoint, route_hint))
        .collect();
    PnServerInfo::new_with_primary_address(id.to_string(), primary, addresses)
        .with_port_mapping(pn_server_port_mapping(port_mapping))
}

pub fn p2p_endpoint_to_pn_endpoint(
    endpoint: &P2pEndpoint,
    route_hint: Option<&P2pEndpoint>,
) -> Endpoint {
    let addr = endpoint.addr();
    Endpoint::new_with_protocol(
        pn_endpoint_protocol(endpoint.protocol()),
        advertised_ip_for(addr.ip(), route_hint),
        addr.port(),
    )
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
        Protocol::Quic => Endpoint::PROTOCOL_QUIC,
        Protocol::Tcp => Endpoint::PROTOCOL_TCP,
        Protocol::Ext(_) => Endpoint::PROTOCOL_QUIC,
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

pub struct ConfigPnServerSelector {
    pn_servers: Vec<PnServerInfo>,
    remote_pn_servers: Mutex<HashMap<String, (PnServerInfo, Instant)>>,
    remote_ttl: Duration,
    store_factory: Option<Arc<SqliteStoreFactory>>,
}

pub type ConfigPnServerSelectorRef = Arc<ConfigPnServerSelector>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyNodeState {
    pub pn_server: PnServerInfo,
    pub status: ProxyNodeApprovalStatus,
    pub live: bool,
    pub updated_at: u64,
    pub comment: String,
}

impl ConfigPnServerSelector {
    pub fn new(pn_servers: Vec<PnServerInfo>) -> Self {
        Self::new_with_remote_ttl(pn_servers, Duration::from_secs(15))
    }

    pub fn new_with_remote_ttl(pn_servers: Vec<PnServerInfo>, remote_ttl: Duration) -> Self {
        Self {
            pn_servers,
            remote_pn_servers: Mutex::new(HashMap::new()),
            remote_ttl,
            store_factory: None,
        }
    }

    pub fn new_with_store(
        pn_servers: Vec<PnServerInfo>,
        store_factory: Arc<SqliteStoreFactory>,
    ) -> Self {
        Self::new_with_store_and_remote_ttl(pn_servers, store_factory, Duration::from_secs(15))
    }

    pub fn new_with_store_and_remote_ttl(
        pn_servers: Vec<PnServerInfo>,
        store_factory: Arc<SqliteStoreFactory>,
        remote_ttl: Duration,
    ) -> Self {
        Self {
            pn_servers,
            remote_pn_servers: Mutex::new(HashMap::new()),
            remote_ttl,
            store_factory: Some(store_factory),
        }
    }

    pub fn start_remote_liveness_monitor(self: &Arc<Self>) {
        let selector = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(selector.remote_liveness_check_interval());
            loop {
                ticker.tick().await;
                selector.prune_expired_remote_pn_servers_now();
            }
        });
    }

    fn remote_liveness_check_interval(&self) -> Duration {
        let mut interval = self.remote_ttl / 3;
        if interval.is_zero() {
            interval = Duration::from_millis(1);
        }
        interval.min(Duration::from_secs(5))
    }

    fn prune_expired_remote_pn_servers_now(&self) {
        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        self.prune_expired_remote_pn_servers(&mut remote_pn_servers);
    }

    fn prune_expired_remote_pn_servers(
        &self,
        remote_pn_servers: &mut HashMap<String, (PnServerInfo, Instant)>,
    ) {
        let now = Instant::now();
        let remote_ttl = self.remote_ttl;
        remote_pn_servers.retain(|_, (pn_server, last_seen)| {
            let live = now.duration_since(*last_seen) <= remote_ttl;
            if !live {
                log_proxy_node_offline(pn_server, now.duration_since(*last_seen));
            }
            live
        });
    }

    fn live_remote_pn_servers(&self) -> Vec<PnServerInfo> {
        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        self.prune_expired_remote_pn_servers(&mut remote_pn_servers);
        remote_pn_servers
            .values()
            .map(|(pn_server, _)| pn_server.clone())
            .collect()
    }

    fn live_remote_pn_server(&self, id: &str) -> Option<PnServerInfo> {
        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        self.prune_expired_remote_pn_servers(&mut remote_pn_servers);
        remote_pn_servers
            .get(id)
            .map(|(pn_server, _)| pn_server.clone())
    }

    fn is_same_pn_server_id(pn_server: &PnServerInfo, id: &str) -> bool {
        pn_server.id == id
    }

    fn is_same_connectable_pn_server(left: &PnServerInfo, right: &PnServerInfo) -> bool {
        left.id == right.id && left.name == right.name && left.endpoints == right.endpoints
    }

    fn client_visible_pn_server(mut pn_server: PnServerInfo) -> PnServerInfo {
        pn_server.port_mapping = None;
        pn_server
    }

    fn primary_endpoint_ip(pn_server: &PnServerInfo) -> Option<IpAddr> {
        pn_server.primary_endpoint().map(|endpoint| endpoint.ip)
    }

    fn mapped_endpoint_port(
        endpoint: &Endpoint,
        port_mapping: Option<&PnServerPortMapping>,
    ) -> u16 {
        let Some(port_mapping) = port_mapping else {
            return endpoint.port;
        };
        match endpoint.protocol.as_str() {
            Endpoint::PROTOCOL_QUIC => port_mapping.quic.unwrap_or(endpoint.port),
            Endpoint::PROTOCOL_TCP => port_mapping.tcp.unwrap_or(endpoint.port),
            _ => endpoint.port,
        }
    }

    fn rewrite_endpoint_ip(
        endpoint: &Endpoint,
        ip: IpAddr,
        port_mapping: Option<&PnServerPortMapping>,
    ) -> Endpoint {
        Endpoint::new_with_protocol(
            endpoint.protocol.clone(),
            ip,
            Self::mapped_endpoint_port(endpoint, port_mapping),
        )
    }

    fn observed_reported_pn_server(
        observed: &PnServerInfo,
        reported: &PnServerInfo,
    ) -> PnServerInfo {
        let Some(observed_ip) = Self::primary_endpoint_ip(observed) else {
            return reported.clone();
        };
        let port_mapping = reported.port_mapping.as_ref();
        let mut merged = PnServerInfo::new_with_endpoints(
            reported.id.clone(),
            reported.endpoints.clone(),
        );
        for endpoint in &reported.endpoints {
            merged.add_endpoint(Self::rewrite_endpoint_ip(
                endpoint,
                observed_ip,
                port_mapping,
            ));
        }
        merged.port_mapping = reported.port_mapping.clone();
        merged.name = reported.name.clone().or_else(|| observed.name.clone());
        merged
    }

    fn merge_reported_remote_pn_server(&self, pn_server: &PnServerInfo) -> PnServerInfo {
        self.remote_pn_servers
            .lock()
            .unwrap()
            .get(&pn_server.id)
            .map(|(reported, _)| reported)
            .map(|observed| Self::observed_reported_pn_server(observed, pn_server))
            .unwrap_or_else(|| pn_server.clone())
    }

    async fn update_remote_heartbeat(&self, pn_server: PnServerInfo) -> VpnResult<()> {
        if let Some(store_factory) = &self.store_factory {
            let mut store = store_factory.get_vpn_store().await?;
            store.ensure_proxy_node_pending(&pn_server).await?;
        }
        let now = Instant::now();
        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        let previous_live_pn_server =
            remote_pn_servers
                .get(&pn_server.id)
                .and_then(|(previous, last_seen)| {
                    let live = now.duration_since(*last_seen) <= self.remote_ttl;
                    if !live {
                        log_proxy_node_offline(previous, now.duration_since(*last_seen));
                    }
                    live.then_some(previous.clone())
                });
        match previous_live_pn_server {
            Some(previous) if previous.endpoints != pn_server.endpoints => {
                log_proxy_node_address_changed(&previous, &pn_server);
            }
            Some(_) => {}
            None => log_proxy_node_online(&pn_server),
        }
        remote_pn_servers.insert(pn_server.id.clone(), (pn_server, now));
        Ok(())
    }

    pub async fn report_observed_heartbeat(&self, pn_server: &PnServerInfo) -> VpnResult<()> {
        let pn_server = self
            .remote_pn_servers
            .lock()
            .unwrap()
            .get(&pn_server.id)
            .map(|(reported, _)| reported)
            .map(|reported| Self::observed_reported_pn_server(pn_server, reported))
            .unwrap_or_else(|| pn_server.clone());
        self.update_remote_heartbeat(pn_server).await
    }

    pub fn is_live(&self, pn_server: &PnServerInfo) -> bool {
        if self
            .pn_servers
            .iter()
            .any(|server| Self::is_same_pn_server_id(server, &pn_server.id))
        {
            return true;
        }
        self.live_remote_pn_server(&pn_server.id).is_some()
    }

    fn is_same_pn_node_id(pn_server: &PnServerInfo, node_id: &NodeId) -> bool {
        if pn_server.id == node_id.to_base36() {
            return true;
        }
        P2pId::from_str(&pn_server.id)
            .map(|pn_id| pn_id.as_slice() == node_id.as_slice())
            .unwrap_or(false)
    }

    async fn is_remote_approved(&self, pn_server: &PnServerInfo) -> VpnResult<bool> {
        let Some(store_factory) = &self.store_factory else {
            return Ok(true);
        };
        let mut store = store_factory.get_vpn_store().await?;
        store.is_proxy_node_approved(pn_server).await
    }

    pub async fn approve_proxy_node(
        &self,
        pn_server: &PnServerInfo,
        comment: Option<&str>,
    ) -> VpnResult<()> {
        let Some(store_factory) = &self.store_factory else {
            return Ok(());
        };
        let mut store = store_factory.get_vpn_store().await?;
        store
            .set_proxy_node_approval(pn_server, ProxyNodeApprovalStatus::Approved, comment)
            .await
    }

    pub async fn reject_proxy_node(
        &self,
        pn_server: &PnServerInfo,
        comment: Option<&str>,
    ) -> VpnResult<()> {
        let Some(store_factory) = &self.store_factory else {
            return Ok(());
        };
        let mut store = store_factory.get_vpn_store().await?;
        store
            .set_proxy_node_approval(pn_server, ProxyNodeApprovalStatus::Rejected, comment)
            .await
    }

    pub async fn list_proxy_nodes(&self) -> VpnResult<Vec<ProxyNodeState>> {
        let Some(store_factory) = &self.store_factory else {
            return Ok(Vec::new());
        };
        let approvals = {
            let mut store = store_factory.get_vpn_store().await?;
            store.list_proxy_node_approvals().await?
        };
        Ok(approvals
            .into_iter()
            .map(|approval: ProxyNodeApproval| ProxyNodeState {
                live: self.is_live(&approval.pn_server),
                pn_server: Self::client_visible_pn_server(
                    self.live_remote_pn_server(&approval.pn_server.id)
                        .unwrap_or(approval.pn_server),
                ),
                status: approval.status,
                updated_at: approval.updated_at,
                comment: approval.comment,
            })
            .collect())
    }
}

fn format_pn_server_name(pn_server: &PnServerInfo) -> &str {
    pn_server.name.as_deref().unwrap_or("<none>")
}

fn log_proxy_node_online(pn_server: &PnServerInfo) {
    log::info!(
        "proxy node is online id={} name={} endpoints={}",
        pn_server.id,
        format_pn_server_name(pn_server),
        format_pn_server_endpoints(pn_server)
    );
}

fn log_proxy_node_offline(pn_server: &PnServerInfo, offline_after: Duration) {
    log::info!(
        "proxy node is offline id={} name={} endpoints={} offline_after_secs={}",
        pn_server.id,
        format_pn_server_name(pn_server),
        format_pn_server_endpoints(pn_server),
        offline_after.as_secs()
    );
}

fn log_proxy_node_address_changed(previous: &PnServerInfo, current: &PnServerInfo) {
    log::info!(
        "proxy node address changed id={} name={} previous_endpoints={} current_endpoints={}",
        current.id,
        format_pn_server_name(current),
        format_pn_server_endpoints(previous),
        format_pn_server_endpoints(current)
    );
}

fn format_pn_server_endpoints(pn_server: &PnServerInfo) -> String {
    pn_server
        .endpoints
        .iter()
        .map(|endpoint| format!("{}://{}:{}", endpoint.protocol, endpoint.ip, endpoint.port))
        .collect::<Vec<_>>()
        .join(",")
}

#[async_trait::async_trait]
impl PnServerSelector for ConfigPnServerSelector {
    async fn is_valid(&self, pn_server: &PnServerInfo) -> VpnResult<bool> {
        if self
            .pn_servers
            .iter()
            .any(|server| Self::is_same_connectable_pn_server(server, pn_server))
        {
            return Ok(true);
        }
        Ok(self
            .live_remote_pn_server(&pn_server.id)
            .as_ref()
            .is_some_and(|live| Self::is_same_connectable_pn_server(live, pn_server))
            && self.is_remote_approved(pn_server).await?)
    }

    async fn select(&self, network_id: NetworkId) -> VpnResult<Option<PnServerInfo>> {
        let mut pn_servers = self.pn_servers.clone();
        let live_remote_pn_servers = self.live_remote_pn_servers();
        for pn_server in live_remote_pn_servers {
            if self.is_remote_approved(&pn_server).await? {
                pn_servers.push(pn_server);
            }
        }
        if pn_servers.is_empty() {
            return Ok(None);
        }
        pn_servers.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then_with(|| left.endpoints.cmp(&right.endpoints))
        });
        pn_servers.dedup_by(|left, right| left.id == right.id);
        let index = network_id as usize % pn_servers.len();
        Ok(Some(Self::client_visible_pn_server(
            pn_servers[index].clone(),
        )))
    }

    async fn matches_pn_node(
        &self,
        pn_server: &PnServerInfo,
        pn_node_id: &NodeId,
    ) -> VpnResult<bool> {
        Ok(Self::is_same_pn_node_id(pn_server, pn_node_id))
    }

    async fn can_accept_connections_from(&self, pn_node_id: &NodeId) -> VpnResult<bool> {
        if self
            .pn_servers
            .iter()
            .any(|server| Self::is_same_pn_node_id(server, pn_node_id))
        {
            return Ok(true);
        }

        for pn_server in self.live_remote_pn_servers() {
            if Self::is_same_pn_node_id(&pn_server, pn_node_id) {
                return self.is_remote_approved(&pn_server).await;
            }
        }
        Ok(false)
    }

    async fn report_heartbeat(&self, pn_server: &PnServerInfo) -> VpnResult<()> {
        let pn_server = self.merge_reported_remote_pn_server(pn_server);
        self.update_remote_heartbeat(pn_server).await
    }
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

    fn pn_server_id_for(node_id: &NodeId) -> String {
        P2pId::from(node_id.as_slice()).to_string()
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
            port_mapping: PnPortMappingConfig::default(),
        };
        let disabled_pn = PnServerConfig {
            enabled: false,
            control_server: None,
            report_interval_secs: 5,
            port_mapping: PnPortMappingConfig::default(),
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
            port_mapping: PnPortMappingConfig::default(),
        };
        let disabled_pn = PnServerConfig {
            enabled: false,
            control_server: None,
            report_interval_secs: 5,
            port_mapping: PnPortMappingConfig::default(),
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
            port_mapping: PnPortMappingConfig::default(),
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
            port_mapping: PnPortMappingConfig::default(),
        };
        let sn_endpoint = parse_quic_endpoint("127.0.0.1:3624").unwrap();

        let endpoints = resolve_service_endpoints(sn_endpoint.clone(), &sn, &pn);
        assert_eq!(endpoints.len(), 2);

        let endpoints = resolve_service_endpoints(sn_endpoint, &disabled_sn, &pn);
        assert_eq!(endpoints.len(), 2);
    }

    #[test]
    fn endpoint_to_pn_server_replaces_unspecified_ip_with_route_local_ip() {
        let endpoint = parse_quic_endpoint("0.0.0.0:4600").unwrap();
        let route_hint = parse_quic_endpoint("127.0.0.1:3624").unwrap();

        let pn_server = endpoint_to_pn_server("remote-node-id", &endpoint, Some(&route_hint));

        assert_eq!(
            pn_server.endpoints,
            vec![Endpoint::new("127.0.0.1".parse::<IpAddr>().unwrap(), 4600)]
        );
    }

    #[test]
    fn endpoints_to_pn_server_reports_listen_ports_and_port_mapping() {
        let quic_endpoint = parse_quic_endpoint("127.0.0.1:3624").unwrap();
        let tcp_endpoint = P2pEndpoint::from((Protocol::Tcp, *quic_endpoint.addr()));
        let port_mapping = PnPortMappingConfig {
            quic: Some(43624),
            tcp: Some(443),
        };

        let pn_server = endpoints_to_pn_server(
            "remote-node-id",
            &quic_endpoint,
            &[quic_endpoint, tcp_endpoint],
            None,
            &port_mapping,
        );

        assert_eq!(
            pn_server.endpoints,
            vec![
                Endpoint::new_with_protocol(
                    Endpoint::PROTOCOL_QUIC,
                    "127.0.0.1".parse().unwrap(),
                    3624,
                ),
                Endpoint::new_tcp("127.0.0.1".parse().unwrap(), 3624),
            ]
        );
        assert_eq!(
            pn_server.port_mapping,
            Some(PnServerPortMapping {
                quic: Some(43624),
                tcp: Some(443),
            })
        );
    }

    #[tokio::test]
    async fn remote_proxy_heartbeat_adds_temporary_selectable_proxy() {
        let selector =
            ConfigPnServerSelector::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let remote_proxy = PnServerInfo::new(
            "remote-node-id".to_string(),
            "127.0.0.1".parse().unwrap(),
            4600,
        );

        assert_eq!(selector.select(1).await.unwrap(), None);
        assert!(!selector.is_valid(&remote_proxy).await.unwrap());

        selector.report_heartbeat(&remote_proxy).await.unwrap();

        assert!(selector.is_valid(&remote_proxy).await.unwrap());
        assert_eq!(selector.select(1).await.unwrap(), Some(remote_proxy));
    }

    #[tokio::test]
    async fn remote_proxy_heartbeat_expires_from_selection() {
        let selector =
            ConfigPnServerSelector::new_with_remote_ttl(Vec::new(), Duration::from_millis(5));
        let remote_proxy = PnServerInfo::new(
            "remote-node-id".to_string(),
            "127.0.0.1".parse().unwrap(),
            4600,
        );

        selector.report_heartbeat(&remote_proxy).await.unwrap();
        assert!(selector.is_valid(&remote_proxy).await.unwrap());

        tokio::time::sleep(Duration::from_millis(15)).await;

        assert!(!selector.is_valid(&remote_proxy).await.unwrap());
        assert_eq!(selector.select(1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn remote_proxy_observed_address_survives_reported_heartbeat() {
        let selector =
            ConfigPnServerSelector::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let observed_proxy = PnServerInfo::new(
            "remote-node-id".to_string(),
            "127.0.0.1".parse().unwrap(),
            4600,
        );
        let reported_proxy =
            PnServerInfo::new(observed_proxy.id.clone(), "10.0.0.2".parse().unwrap(), 3624)
                .with_name(Some("remote-proxy".to_string()))
                .with_port_mapping(Some(PnServerPortMapping {
                    quic: Some(443),
                    tcp: None,
                }));

        selector
            .report_observed_heartbeat(&observed_proxy)
            .await
            .unwrap();
        selector.report_heartbeat(&reported_proxy).await.unwrap();

        let selected = selector.select(1).await.unwrap().unwrap();
        assert_eq!(selected.name.as_deref(), Some("remote-proxy"));
        assert_eq!(
            selected.endpoints,
            vec![
                Endpoint::new("10.0.0.2".parse().unwrap(), 3624),
                Endpoint::new("127.0.0.1".parse().unwrap(), 443),
            ]
        );
    }

    #[tokio::test]
    async fn remote_proxy_observed_address_replaces_reported_heartbeat() {
        let selector =
            ConfigPnServerSelector::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let reported_proxy = PnServerInfo::new(
            "remote-node-id".to_string(),
            "10.0.0.2".parse().unwrap(),
            3624,
        )
        .with_name(Some("remote-proxy".to_string()))
        .with_port_mapping(Some(PnServerPortMapping {
            quic: Some(443),
            tcp: None,
        }));
        let observed_proxy = PnServerInfo::new(
            reported_proxy.id.clone(),
            "127.0.0.1".parse().unwrap(),
            56000,
        );

        selector.report_heartbeat(&reported_proxy).await.unwrap();
        selector
            .report_observed_heartbeat(&observed_proxy)
            .await
            .unwrap();

        let selected = selector.select(1).await.unwrap().unwrap();
        assert_eq!(selected.name.as_deref(), Some("remote-proxy"));
        assert_eq!(
            selected.endpoints,
            vec![
                Endpoint::new("10.0.0.2".parse().unwrap(), 3624),
                Endpoint::new("127.0.0.1".parse().unwrap(), 443),
            ]
        );
    }

    #[tokio::test]
    async fn remote_proxy_observed_address_survives_reported_heartbeat_in_store() {
        let db_dir = new_temp_dir();
        let db_path = db_dir.join("vpn.db");
        let store_factory = Arc::new(
            SqliteStoreFactory::create(db_path.to_str().unwrap())
                .await
                .unwrap(),
        );
        {
            let mut store = store_factory.get_vpn_store().await.unwrap();
            store.init_db().await.unwrap();
        }
        let selector = ConfigPnServerSelector::new_with_store_and_remote_ttl(
            Vec::new(),
            store_factory.clone(),
            Duration::from_secs(30),
        );
        let observed_proxy = PnServerInfo::new(
            "remote-node-id".to_string(),
            "127.0.0.1".parse().unwrap(),
            4600,
        );
        let reported_proxy =
            PnServerInfo::new(observed_proxy.id.clone(), "10.0.0.2".parse().unwrap(), 3624)
                .with_name(Some("remote-proxy".to_string()))
                .with_port_mapping(Some(PnServerPortMapping {
                    quic: Some(443),
                    tcp: None,
                }));

        selector
            .report_observed_heartbeat(&observed_proxy)
            .await
            .unwrap();
        selector.report_heartbeat(&reported_proxy).await.unwrap();

        let nodes = selector.list_proxy_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].pn_server.name.as_deref(), Some("remote-proxy"));
        assert_eq!(
            nodes[0].pn_server.endpoints,
            vec![
                Endpoint::new("10.0.0.2".parse().unwrap(), 3624),
                Endpoint::new("127.0.0.1".parse().unwrap(), 443),
            ]
        );
    }

    #[tokio::test]
    async fn local_proxy_node_can_accept_connections_without_heartbeat() {
        let local_node = NodeId::from(vec![9u8; 32].as_slice());
        let local_proxy = PnServerInfo::new(
            pn_server_id_for(&local_node),
            "127.0.0.1".parse().unwrap(),
            4600,
        );
        let selector = ConfigPnServerSelector::new_with_remote_ttl(
            vec![local_proxy.clone()],
            Duration::from_secs(30),
        );

        assert!(
            selector
                .can_accept_connections_from(&local_node)
                .await
                .unwrap()
        );
        assert!(
            selector
                .matches_pn_node(&local_proxy, &local_node)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn remote_proxy_node_must_be_live_and_approved_to_accept_connections() {
        let db_dir = new_temp_dir();
        let db_path = db_dir.join("vpn.db");
        let store_factory = Arc::new(
            SqliteStoreFactory::create(db_path.to_str().unwrap())
                .await
                .unwrap(),
        );
        {
            let mut store = store_factory.get_vpn_store().await.unwrap();
            store.init_db().await.unwrap();
        }
        let selector = ConfigPnServerSelector::new_with_store_and_remote_ttl(
            Vec::new(),
            store_factory.clone(),
            Duration::from_secs(30),
        );
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let remote_proxy = PnServerInfo::new(
            pn_server_id_for(&remote_node),
            "127.0.0.1".parse().unwrap(),
            4700,
        );

        assert!(
            !selector
                .can_accept_connections_from(&remote_node)
                .await
                .unwrap()
        );

        selector.report_heartbeat(&remote_proxy).await.unwrap();
        assert!(
            !selector
                .can_accept_connections_from(&remote_node)
                .await
                .unwrap()
        );

        selector
            .approve_proxy_node(&remote_proxy, Some("ok"))
            .await
            .unwrap();
        assert!(
            selector
                .can_accept_connections_from(&remote_node)
                .await
                .unwrap()
        );

        selector
            .reject_proxy_node(&remote_proxy, Some("no"))
            .await
            .unwrap();
        assert!(
            !selector
                .can_accept_connections_from(&remote_node)
                .await
                .unwrap()
        );

        drop(selector);
        drop(store_factory);
        let _ = fs::remove_dir_all(db_dir);
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

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_can_configure_pn_port_mapping() {
        let dir = new_temp_dir();
        fs::write(
            dir.join(DEFAULT_YAML_CONFIG),
            r#"
pn:
  port_mapping:
    quic: 43624
    tcp: 443
"#,
        )
        .unwrap();

        let config = build_server_config(None, &dir).unwrap();
        let pn_config = get_pn_server_config(&config).unwrap();

        assert_eq!(
            pn_config.port_mapping,
            PnPortMappingConfig {
                quic: Some(43624),
                tcp: Some(443),
            }
        );

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
