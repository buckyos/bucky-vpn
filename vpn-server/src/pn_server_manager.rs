use crate::pn_server_info::{
    PnServerEndpoint, PnServerInfoPayload, PnServerPortMapping, decode_pn_server_info,
    encode_pn_server_info,
};
use crate::sqlite_store_factory::{ProxyNodeApproval, ProxyNodeApprovalStatus, SqliteStoreFactory};
use p2p_frame::p2p_identity::P2pId;
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vpn_frame::{PnServerInfo, ProxyNodeHeartbeat};
use vpn_frame::errors::VpnResult;
use vpn_frame::server::{NetworkId, NodeId, PnServerSelector, VpnStoreFactory};

pub struct PnServerManager {
    pn_servers: Vec<PnServerInfo>,
    remote_pn_servers: Mutex<HashMap<String, RemotePnServerState>>,
    remote_ttl: Duration,
    store_factory: Option<Arc<SqliteStoreFactory>>,
}

pub type PnServerManagerRef = Arc<PnServerManager>;

#[derive(Clone, Debug)]
struct RemotePnServerState {
    reported: Option<PnServerInfo>,
    observed: Option<PnServerInfo>,
    current: PnServerInfo,
    last_heartbeat: Option<Instant>,
    offline_logged: bool,
}

impl RemotePnServerState {
    fn new_reported(reported: PnServerInfo, now: Instant) -> Self {
        let current = PnServerManager::merge_remote_pn_server(None, Some(&reported));
        Self {
            current,
            reported: Some(reported),
            observed: None,
            last_heartbeat: Some(now),
            offline_logged: false,
        }
    }

    fn new_observed(observed: PnServerInfo) -> Self {
        let current = PnServerManager::merge_remote_pn_server(Some(&observed), None);
        Self {
            current,
            reported: None,
            observed: Some(observed),
            last_heartbeat: None,
            offline_logged: false,
        }
    }

    fn update_reported(&mut self, reported: PnServerInfo, now: Instant) {
        self.reported = Some(reported);
        self.refresh_current();
        self.last_heartbeat = Some(now);
        self.offline_logged = false;
    }

    fn update_observed(&mut self, observed: PnServerInfo) {
        self.observed = Some(observed);
        self.refresh_current();
    }

    fn refresh_current(&mut self) {
        self.current =
            PnServerManager::merge_remote_pn_server(self.observed.as_ref(), self.reported.as_ref());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyNodeState {
    pub pn_server: PnServerInfo,
    pub observed_addr: Option<PnServerEndpoint>,
    pub status: ProxyNodeApprovalStatus,
    pub live: bool,
    pub updated_at: u64,
    pub comment: String,
}

impl PnServerManager {
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
        remote_pn_servers: &mut HashMap<String, RemotePnServerState>,
    ) {
        let now = Instant::now();
        remote_pn_servers.retain(|_, state| {
            let expired = self.mark_remote_state_offline_if_needed(state, now);
            !expired || state.observed.is_some()
        });
    }

    fn mark_remote_state_offline_if_needed(
        &self,
        state: &mut RemotePnServerState,
        now: Instant,
    ) -> bool {
        let Some(last_heartbeat) = state.last_heartbeat else {
            return false;
        };
        let heartbeat_age = now.duration_since(last_heartbeat);
        if heartbeat_age <= self.remote_ttl {
            return false;
        }
        if !state.offline_logged {
            log_proxy_node_offline(&state.current, heartbeat_age);
            state.offline_logged = true;
        }
        true
    }

    fn remote_state_is_live(&self, state: &RemotePnServerState, now: Instant) -> bool {
        state
            .last_heartbeat
            .is_some_and(|last| now.duration_since(last) <= self.remote_ttl)
    }

    fn remote_state_is_usable(&self, state: &RemotePnServerState, now: Instant) -> bool {
        self.remote_state_is_live(state, now) && Self::has_connectable_endpoint(&state.current)
    }

    fn live_remote_pn_servers(&self) -> Vec<PnServerInfo> {
        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        self.prune_expired_remote_pn_servers(&mut remote_pn_servers);
        remote_pn_servers
            .values()
            .filter(|state| self.remote_state_is_live(state, Instant::now()))
            .map(|state| state.current.clone())
            .collect()
    }

    fn live_remote_pn_server(&self, id: &str) -> Option<PnServerInfo> {
        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        self.prune_expired_remote_pn_servers(&mut remote_pn_servers);
        let now = Instant::now();
        remote_pn_servers
            .get(id)
            .filter(|state| self.remote_state_is_live(state, now))
            .map(|state| state.current.clone())
    }

    fn in_memory_proxy_node(
        &self,
        id: &str,
    ) -> (Option<PnServerInfo>, Option<PnServerEndpoint>) {
        if let Some(local) = self
            .pn_servers
            .iter()
            .find(|server| Self::is_same_pn_server_id(server, id))
        {
            return (Some(local.clone()), None);
        }

        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        self.prune_expired_remote_pn_servers(&mut remote_pn_servers);
        let Some(state) = remote_pn_servers.get(id) else {
            return (None, None);
        };
        if !self.remote_state_is_live(state, Instant::now()) {
            return (None, None);
        }
        let observed_addr = state
            .observed
            .as_ref()
            .and_then(|observed| decode_pn_server_info(observed).ok())
            .and_then(|payload| payload.primary_endpoint().cloned());
        (Some(state.current.clone()), observed_addr)
    }

    fn is_same_pn_server_id(pn_server: &PnServerInfo, id: &str) -> bool {
        pn_server.id == id
    }

    fn is_same_connectable_pn_server(left: &PnServerInfo, right: &PnServerInfo) -> bool {
        let left_payload = decode_pn_server_info(left).unwrap_or_default();
        let right_payload = decode_pn_server_info(right).unwrap_or_default();
        left.id == right.id
            && left_payload.name == right_payload.name
            && left_payload.endpoints == right_payload.endpoints
    }

    fn client_visible_pn_server(pn_server: PnServerInfo) -> PnServerInfo {
        let mut payload = decode_pn_server_info(&pn_server).unwrap_or_default();
        payload.endpoints.retain(|endpoint| !endpoint.ip.is_unspecified());
        for endpoint in &mut payload.endpoints {
            if let Some(mapped_port) =
                Self::mapped_endpoint_port(endpoint, payload.port_mapping.as_ref())
            {
                endpoint.port = mapped_port;
            }
        }
        payload.port_mapping = None;
        payload.advertised_ip = None;
        encode_pn_server_info(pn_server.id, payload).unwrap()
    }

    fn primary_endpoint_ip(pn_server: &PnServerInfo) -> Option<IpAddr> {
        decode_pn_server_info(pn_server)
            .ok()
            .and_then(|payload| payload.primary_endpoint().map(|endpoint| endpoint.ip))
    }

    fn mapped_endpoint_port(
        endpoint: &PnServerEndpoint,
        port_mapping: Option<&PnServerPortMapping>,
    ) -> Option<u16> {
        let port_mapping = port_mapping?;
        match endpoint.protocol.as_str() {
            PnServerEndpoint::PROTOCOL_QUIC => port_mapping.quic,
            PnServerEndpoint::PROTOCOL_TCP => port_mapping.tcp,
            _ => None,
        }
    }

    fn rewrite_endpoint_ip(
        endpoint: &PnServerEndpoint,
        ip: IpAddr,
        port_mapping: Option<&PnServerPortMapping>,
    ) -> Option<PnServerEndpoint> {
        Some(PnServerEndpoint::new_with_protocol(
            endpoint.protocol.clone(),
            ip,
            Self::mapped_endpoint_port(endpoint, port_mapping).unwrap_or(endpoint.port),
        ))
    }

    fn observed_reported_pn_server(
        observed: &PnServerInfo,
        reported: &PnServerInfo,
    ) -> PnServerInfo {
        let observed_payload = decode_pn_server_info(observed).unwrap_or_default();
        let reported_payload = decode_pn_server_info(reported).unwrap_or_default();
        let mut merged = PnServerInfoPayload::default();
        let observed_ip = Self::primary_endpoint_ip(observed);
        if let (Some(advertised_ip), Some(observed_ip)) =
            (reported_payload.advertised_ip, observed_ip)
        {
            if advertised_ip != observed_ip {
                log::warn!(
                    "proxy node advertised IP differs from observed IP id={} advertised_ip={} observed_ip={}",
                    reported.id,
                    advertised_ip,
                    observed_ip
                );
            }
        }
        if let Some(endpoint_ip) = reported_payload.advertised_ip.or(observed_ip) {
            for endpoint in &reported_payload.endpoints {
                if let Some(endpoint) = Self::rewrite_endpoint_ip(
                    endpoint,
                    endpoint_ip,
                    reported_payload.port_mapping.as_ref(),
                ) {
                    merged.add_endpoint(endpoint);
                }
            }
        }
        merged.advertised_ip = reported_payload.advertised_ip;
        merged.port_mapping = reported_payload.port_mapping.clone();
        merged.name = reported_payload.name.or(observed_payload.name);
        encode_pn_server_info(reported.id.clone(), merged).unwrap()
    }

    fn merge_remote_pn_server(
        observed: Option<&PnServerInfo>,
        reported: Option<&PnServerInfo>,
    ) -> PnServerInfo {
        match (observed, reported) {
            (Some(observed), Some(reported)) => {
                Self::observed_reported_pn_server(observed, reported)
            }
            (Some(observed), None) => {
                let observed_payload = decode_pn_server_info(observed).unwrap_or_default();
                encode_pn_server_info(
                    observed.id.clone(),
                    PnServerInfoPayload::default().with_name(observed_payload.name),
                )
                .unwrap()
            }
            (None, Some(reported)) => {
                let reported_payload = decode_pn_server_info(reported).unwrap_or_default();
                let mut current = PnServerInfoPayload::default().with_name(reported_payload.name);
                if let Some(advertised_ip) = reported_payload.advertised_ip {
                    for endpoint in &reported_payload.endpoints {
                        if let Some(endpoint) = Self::rewrite_endpoint_ip(
                            endpoint,
                            advertised_ip,
                            reported_payload.port_mapping.as_ref(),
                        ) {
                            current.add_endpoint(endpoint);
                        }
                    }
                    current.advertised_ip = Some(advertised_ip);
                }
                current.port_mapping = reported_payload.port_mapping;
                encode_pn_server_info(reported.id.clone(), current).unwrap()
            }
            (None, None) => unreachable!("remote proxy state must have reported or observed data"),
        }
    }

    fn pn_server_endpoints(pn_server: &PnServerInfo) -> Vec<PnServerEndpoint> {
        decode_pn_server_info(pn_server)
            .map(|payload| payload.endpoints)
            .unwrap_or_default()
    }

    fn has_connectable_endpoint(pn_server: &PnServerInfo) -> bool {
        Self::pn_server_endpoints(pn_server)
            .iter()
            .any(|endpoint| !endpoint.ip.is_unspecified())
    }

    async fn persist_remote_heartbeat(&self, pn_server: &PnServerInfo) -> VpnResult<()> {
        if let Some(store_factory) = &self.store_factory {
            let mut store = store_factory.get_vpn_store().await?;
            store.ensure_proxy_node_pending(pn_server).await?;
        }
        Ok(())
    }

    fn update_heartbeat_with_observation(
        &self,
        pn_node_id: &NodeId,
        heartbeat: &ProxyNodeHeartbeat,
        observation: Option<&PnServerInfo>,
    ) -> VpnResult<PnServerInfo> {
        if let Some(observation) = observation {
            if !Self::is_same_pn_node_id(observation, pn_node_id) {
                return Err(vpn_frame::errors::vpn_err!(
                    vpn_frame::errors::VpnErrorCode::InvalidParam,
                    "observed proxy {} does not match heartbeat peer {}",
                    observation.id,
                    pn_node_id.to_base36()
                ));
            }
        }

        let now = Instant::now();
        let id = heartbeat
            .pn_server
            .as_ref()
            .map(|pn_server| pn_server.id.clone())
            .or_else(|| observation.map(|pn_server| pn_server.id.clone()))
            .unwrap_or_else(|| P2pId::from(pn_node_id.as_slice()).to_string());
        let mut remote_pn_servers = self.remote_pn_servers.lock().unwrap();
        let (previous_usable, previous_pn_server) = remote_pn_servers
            .get_mut(&id)
            .map(|previous| {
                self.mark_remote_state_offline_if_needed(previous, now);
                (
                    self.remote_state_is_usable(previous, now),
                    previous.current.clone(),
                )
            })
            .unwrap_or_else(|| (false, PnServerInfo::new(id.clone(), Vec::new())));

        if !remote_pn_servers.contains_key(&id) {
            let initial = if let Some(reported) = heartbeat.pn_server.as_ref() {
                RemotePnServerState::new_reported(reported.clone(), now)
            } else if let Some(observed) = observation {
                RemotePnServerState::new_observed(observed.clone())
            } else {
                return Err(vpn_frame::errors::vpn_err!(
                    vpn_frame::errors::VpnErrorCode::InvalidParam,
                    "proxy heartbeat {} has no registered metadata",
                    id
                ));
            };
            remote_pn_servers.insert(id.clone(), initial);
        }

        let state = remote_pn_servers.get_mut(&id).unwrap();
        if let Some(observed) = observation {
            state.update_observed(observed.clone());
        }
        if let Some(reported) = heartbeat.pn_server.as_ref() {
            state.update_reported(reported.clone(), now);
        } else {
            state.last_heartbeat = Some(now);
            state.offline_logged = false;
        }

        let current = state.current.clone();
        let current_usable = self.remote_state_is_usable(state, now);
        if previous_usable && current_usable {
            if Self::pn_server_endpoints(&previous_pn_server) != Self::pn_server_endpoints(&current)
            {
                log_proxy_node_address_changed(&previous_pn_server, &current);
            }
        } else if current_usable {
            log_proxy_node_online(&current);
        } else if previous_usable {
            log_proxy_node_offline(&previous_pn_server, Duration::ZERO);
        }
        Ok(current)
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
            .map(|approval: ProxyNodeApproval| {
                let (pn_server, observed_addr) =
                    self.in_memory_proxy_node(&approval.pn_server_id);
                let live = pn_server.is_some();
                ProxyNodeState {
                    pn_server: Self::client_visible_pn_server(pn_server.unwrap_or_else(|| {
                        PnServerInfo::new(approval.pn_server_id.clone(), Vec::new())
                    })),
                    observed_addr,
                    live,
                    status: approval.status,
                    updated_at: approval.updated_at,
                    comment: approval.comment,
                }
            })
            .collect())
    }
}

fn format_pn_server_name(_pn_server: &PnServerInfo) -> &str {
    "<opaque>"
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
    decode_pn_server_info(pn_server)
        .map(|payload| payload.endpoints)
        .unwrap_or_default()
        .iter()
        .map(|endpoint| format!("{}://{}:{}", endpoint.protocol, endpoint.ip, endpoint.port))
        .collect::<Vec<_>>()
        .join(",")
}

#[async_trait::async_trait]
impl PnServerSelector for PnServerManager {
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
            .is_some_and(|live| {
                Self::has_connectable_endpoint(live)
                    && Self::is_same_connectable_pn_server(live, pn_server)
            })
            && self.is_remote_approved(pn_server).await?)
    }

    async fn select(&self, network_id: NetworkId) -> VpnResult<Option<PnServerInfo>> {
        let mut pn_servers = self
            .pn_servers
            .iter()
            .filter(|pn_server| Self::has_connectable_endpoint(pn_server))
            .cloned()
            .collect::<Vec<_>>();
        let live_remote_pn_servers = self.live_remote_pn_servers();
        for pn_server in live_remote_pn_servers {
            if Self::has_connectable_endpoint(&pn_server)
                && self.is_remote_approved(&pn_server).await?
            {
                pn_servers.push(pn_server);
            }
        }
        if pn_servers.is_empty() {
            return Ok(None);
        }
        pn_servers.sort_by(|left, right| {
            left.id.cmp(&right.id).then_with(|| {
                Self::pn_server_endpoints(left).cmp(&Self::pn_server_endpoints(right))
            })
        });
        pn_servers.dedup_by(|left, right| left.id == right.id);
        let index = network_id as usize % pn_servers.len();
        Ok(Some(Self::client_visible_pn_server(
            pn_servers[index].clone(),
        )))
    }

    async fn resolve(&self, pn_server: &PnServerInfo) -> VpnResult<Option<PnServerInfo>> {
        if let Some(local) = self
            .pn_servers
            .iter()
            .find(|server| {
                Self::is_same_pn_server_id(server, &pn_server.id)
                    && Self::has_connectable_endpoint(server)
            })
        {
            return Ok(Some(Self::client_visible_pn_server(local.clone())));
        }

        if let Some(remote) = self.live_remote_pn_server(&pn_server.id) {
            if Self::has_connectable_endpoint(&remote)
                && self.is_remote_approved(&remote).await?
            {
                return Ok(Some(Self::client_visible_pn_server(remote)));
            }
        }

        Ok(None)
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

    async fn report_heartbeat(
        &self,
        pn_node_id: &NodeId,
        heartbeat: &ProxyNodeHeartbeat,
    ) -> VpnResult<()> {
        let current = self.update_heartbeat_with_observation(pn_node_id, heartbeat, None)?;
        self.persist_remote_heartbeat(&current).await
    }

    async fn report_heartbeat_with_observation(
        &self,
        pn_node_id: &NodeId,
        heartbeat: &ProxyNodeHeartbeat,
        observation: Option<&PnServerInfo>,
    ) -> VpnResult<()> {
        let current = self.update_heartbeat_with_observation(pn_node_id, heartbeat, observation)?;
        self.persist_remote_heartbeat(&current).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_store_factory::SqliteStoreFactory;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    fn new_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bucky-vpn-pn-server-manager-{}-{}-{}",
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

    fn pn_server(id: impl Into<String>, ip: IpAddr, port: u16) -> PnServerInfo {
        encode_pn_server_info(
            id,
            PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(ip, port)),
        )
        .unwrap()
    }

    fn pn_server_with_payload(id: impl Into<String>, payload: PnServerInfoPayload) -> PnServerInfo {
        encode_pn_server_info(id, payload).unwrap()
    }

    fn payload(pn_server: &PnServerInfo) -> PnServerInfoPayload {
        decode_pn_server_info(pn_server).unwrap()
    }

    async fn heartbeat(selector: &PnServerManager, pn_server: &PnServerInfo) {
        let pn_node_id = NodeId::from_base36_or_base58(&pn_server.id)
            .unwrap_or_else(|_| NodeId::from(vec![9u8; 32].as_slice()));
        selector
            .report_heartbeat(
                &pn_node_id,
                &ProxyNodeHeartbeat {
                    heartbeat_id: vpn_frame::ProxyNodeHeartbeatId("test-heartbeat".to_string()),
                    pn_server: Some(pn_server.clone()),
                },
            )
            .await
            .unwrap();
    }

    async fn heartbeat_with_observation(
        selector: &PnServerManager,
        pn_server: &PnServerInfo,
        observation: &PnServerInfo,
    ) {
        let pn_node_id = NodeId::from_base36_or_base58(&pn_server.id)
            .unwrap_or_else(|_| NodeId::from(vec![9u8; 32].as_slice()));
        selector
            .report_heartbeat_with_observation(
                &pn_node_id,
                &ProxyNodeHeartbeat {
                    heartbeat_id: vpn_frame::ProxyNodeHeartbeatId("test-heartbeat".to_string()),
                    pn_server: Some(pn_server.clone()),
                },
                Some(observation),
            )
            .await
            .unwrap();
    }

    async fn observed_heartbeat(selector: &PnServerManager, observation: &PnServerInfo) {
        let pn_node_id = NodeId::from_base36_or_base58(&observation.id).unwrap();
        selector
            .report_heartbeat_with_observation(
                &pn_node_id,
                &ProxyNodeHeartbeat {
                    heartbeat_id: vpn_frame::ProxyNodeHeartbeatId("test-heartbeat".to_string()),
                    pn_server: None,
                },
                Some(observation),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reported_only_proxy_is_not_selectable() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let remote_proxy = pn_server("remote-node-id", "127.0.0.1".parse().unwrap(), 4600);

        assert_eq!(selector.select(1).await.unwrap(), None);
        assert!(!selector.is_valid(&remote_proxy).await.unwrap());

        heartbeat(&selector, &remote_proxy).await;

        assert!(!selector.is_valid(&remote_proxy).await.unwrap());
        assert_eq!(selector.select(1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn reported_only_proxy_remains_unselectable_after_ttl() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_millis(5));
        let remote_proxy = pn_server("remote-node-id", "127.0.0.1".parse().unwrap(), 4600);

        heartbeat(&selector, &remote_proxy).await;
        assert!(!selector.is_valid(&remote_proxy).await.unwrap());

        tokio::time::sleep(Duration::from_millis(15)).await;

        assert!(!selector.is_valid(&remote_proxy).await.unwrap());
        assert_eq!(selector.select(1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn remote_proxy_observed_address_survives_reported_heartbeat() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let observed_proxy = pn_server(
            pn_server_id_for(&remote_node),
            "127.0.0.1".parse().unwrap(),
            4600,
        );
        let reported_proxy = pn_server_with_payload(
            observed_proxy.id.clone(),
            PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(
                "10.0.0.2".parse().unwrap(),
                3624,
            ))
            .with_name(Some("remote-proxy".to_string()))
            .with_port_mapping(Some(PnServerPortMapping {
                quic: Some(443),
                tcp: None,
            })),
        );

        heartbeat_with_observation(&selector, &reported_proxy, &observed_proxy).await;

        let selected = selector.select(1).await.unwrap().unwrap();
        let selected_payload = payload(&selected);
        assert_eq!(selected_payload.name.as_deref(), Some("remote-proxy"));
        assert_eq!(
            selected_payload.endpoints,
            vec![PnServerEndpoint::new(
                "127.0.0.1".parse().unwrap(),
                443
            )]
        );
    }

    #[tokio::test]
    async fn remote_proxy_advertised_ip_overrides_observed_address() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let advertised_ip = "203.0.113.8".parse::<IpAddr>().unwrap();
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let reported_proxy = pn_server_with_payload(
            pn_server_id_for(&remote_node),
            PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(
                "10.0.0.2".parse().unwrap(),
                3624,
            ))
            .with_name(Some("remote-proxy".to_string()))
            .with_advertised_ip(Some(advertised_ip))
            .with_port_mapping(Some(PnServerPortMapping {
                quic: Some(443),
                tcp: None,
            })),
        );
        let observed_proxy = pn_server(
            reported_proxy.id.clone(),
            "127.0.0.1".parse().unwrap(),
            56000,
        );

        heartbeat_with_observation(&selector, &reported_proxy, &observed_proxy).await;

        let selected = selector.select(1).await.unwrap().unwrap();
        let selected_payload = payload(&selected);
        assert_eq!(selected_payload.name.as_deref(), Some("remote-proxy"));
        assert_eq!(
            selected_payload.endpoints,
            vec![PnServerEndpoint::new(advertised_ip, 443)]
        );
    }

    #[tokio::test]
    async fn remote_proxy_suppressed_local_address_uses_observed_ip_and_mapped_ports() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let reported_proxy = pn_server_with_payload(
            pn_server_id_for(&remote_node),
            PnServerInfoPayload::new_with_primary_address(
                PnServerEndpoint::new_with_protocol(
                    PnServerEndpoint::PROTOCOL_QUIC,
                    "0.0.0.0".parse().unwrap(),
                    3624,
                ),
                vec![PnServerEndpoint::new_tcp("0.0.0.0".parse().unwrap(), 3624)],
            )
            .with_port_mapping(Some(PnServerPortMapping {
                quic: Some(43624),
                tcp: Some(443),
            })),
        );
        let observed_proxy = pn_server(
            reported_proxy.id.clone(),
            "47.113.93.155".parse().unwrap(),
            56000,
        );

        heartbeat_with_observation(&selector, &reported_proxy, &observed_proxy).await;

        let selected = selector.select(1).await.unwrap().unwrap();
        assert_eq!(
            payload(&selected).endpoints,
            vec![
                PnServerEndpoint::new_with_protocol(
                    PnServerEndpoint::PROTOCOL_QUIC,
                    "47.113.93.155".parse().unwrap(),
                    43624,
                ),
                PnServerEndpoint::new_tcp("47.113.93.155".parse().unwrap(), 443),
            ]
        );
    }

    #[tokio::test]
    async fn remote_proxy_without_port_mapping_is_selectable_with_reported_listen_port() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let reported_proxy = pn_server_with_payload(
            pn_server_id_for(&remote_node),
            PnServerInfoPayload::new_with_primary_address(
                PnServerEndpoint::new_with_protocol(
                    PnServerEndpoint::PROTOCOL_QUIC,
                    "172.17.0.5".parse().unwrap(),
                    3625,
                ),
                vec![PnServerEndpoint::new_tcp(
                    "172.17.0.5".parse().unwrap(),
                    3625,
                )],
            ),
        );
        let observed_proxy = pn_server(
            reported_proxy.id.clone(),
            "47.113.93.155".parse().unwrap(),
            56000,
        );

        heartbeat_with_observation(&selector, &reported_proxy, &observed_proxy).await;

        let selected = selector.select(1).await.unwrap().unwrap();
        assert_eq!(
            payload(&selected).endpoints,
            vec![
                PnServerEndpoint::new_with_protocol(
                    PnServerEndpoint::PROTOCOL_QUIC,
                    "47.113.93.155".parse().unwrap(),
                    3625,
                ),
                PnServerEndpoint::new_tcp("47.113.93.155".parse().unwrap(), 3625),
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
        let selector = PnServerManager::new_with_store_and_remote_ttl(
            Vec::new(),
            store_factory.clone(),
            Duration::from_secs(30),
        );
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let observed_proxy = pn_server(
            pn_server_id_for(&remote_node),
            "127.0.0.1".parse().unwrap(),
            4600,
        );
        let reported_proxy = pn_server_with_payload(
            observed_proxy.id.clone(),
            PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(
                "10.0.0.2".parse().unwrap(),
                3624,
            ))
            .with_name(Some("remote-proxy".to_string()))
            .with_port_mapping(Some(PnServerPortMapping {
                quic: Some(443),
                tcp: None,
            })),
        );

        heartbeat_with_observation(&selector, &reported_proxy, &observed_proxy).await;

        let nodes = selector.list_proxy_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        let node_payload = payload(&nodes[0].pn_server);
        assert_eq!(node_payload.name.as_deref(), Some("remote-proxy"));
        assert_eq!(
            node_payload.endpoints,
            vec![PnServerEndpoint::new(
                "127.0.0.1".parse().unwrap(),
                443
            )]
        );
        assert_eq!(
            nodes[0].observed_addr,
            Some(PnServerEndpoint::new(
                "127.0.0.1".parse().unwrap(),
                4600
            ))
        );
    }

    #[tokio::test]
    async fn offline_proxy_node_list_omits_runtime_pn_details() {
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
        let selector = PnServerManager::new_with_store_and_remote_ttl(
            Vec::new(),
            store_factory.clone(),
            Duration::from_millis(1),
        );
        let remote_proxy = pn_server_with_payload(
            "remote-node-id",
            PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(
                "10.0.0.2".parse().unwrap(),
                3624,
            ))
            .with_name(Some("remote-proxy".to_string())),
        );

        heartbeat(&selector, &remote_proxy).await;
        selector
            .approve_proxy_node(&remote_proxy, Some("ok"))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;

        let nodes = selector.list_proxy_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].pn_server.id, "remote-node-id");
        let node_payload = payload(&nodes[0].pn_server);
        assert_eq!(node_payload.name, None);
        assert!(node_payload.endpoints.is_empty());
        assert_eq!(nodes[0].observed_addr, None);
        assert!(!nodes[0].live);
        assert_eq!(nodes[0].status, ProxyNodeApprovalStatus::Approved);
        assert_eq!(nodes[0].comment, "ok");
    }

    #[tokio::test]
    async fn local_proxy_node_applies_port_mapping_without_heartbeat() {
        let local_node = NodeId::from(vec![9u8; 32].as_slice());
        let local_proxy = pn_server_with_payload(
            pn_server_id_for(&local_node),
            PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(
                "127.0.0.1".parse().unwrap(),
                4600,
            ))
            .with_port_mapping(Some(PnServerPortMapping {
                quic: Some(443),
                tcp: None,
            })),
        );
        let selector = PnServerManager::new_with_remote_ttl(
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
        let selected = selector.select(1).await.unwrap().unwrap();
        assert_eq!(
            payload(&selected).endpoints,
            vec![PnServerEndpoint::new(
                "127.0.0.1".parse().unwrap(),
                443
            )]
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
        let selector = PnServerManager::new_with_store_and_remote_ttl(
            Vec::new(),
            store_factory.clone(),
            Duration::from_secs(30),
        );
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let remote_proxy = pn_server(
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

        heartbeat(&selector, &remote_proxy).await;
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

    #[tokio::test]
    async fn observed_only_addressless_proxy_is_not_selectable() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let observed_proxy = pn_server(
            pn_server_id_for(&remote_node),
            "198.51.100.7".parse().unwrap(),
            56000,
        );

        observed_heartbeat(&selector, &observed_proxy).await;

        assert_eq!(selector.select(1).await.unwrap(), None);
    }

    #[tokio::test]
    async fn dedicated_heartbeat_controls_remote_online_state() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_millis(5));
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let remote_proxy = pn_server(
            pn_server_id_for(&remote_node),
            "127.0.0.1".parse().unwrap(),
            4600,
        );

        observed_heartbeat(&selector, &remote_proxy).await;
        assert!(selector.is_live(&remote_proxy));

        tokio::time::sleep(Duration::from_millis(15)).await;
        assert!(!selector.is_live(&remote_proxy));
    }

    #[tokio::test]
    async fn observed_source_port_change_does_not_change_client_endpoint() {
        let selector = PnServerManager::new_with_remote_ttl(Vec::new(), Duration::from_secs(30));
        let remote_node = NodeId::from(vec![7u8; 32].as_slice());
        let reported_proxy = pn_server_with_payload(
            pn_server_id_for(&remote_node),
            PnServerInfoPayload::new_with_endpoint(PnServerEndpoint::new(
                "10.0.0.2".parse().unwrap(),
                3624,
            ))
            .with_port_mapping(Some(PnServerPortMapping {
                quic: Some(443),
                tcp: None,
            })),
        );
        let first_observed = pn_server(
            reported_proxy.id.clone(),
            "198.51.100.7".parse().unwrap(),
            56000,
        );
        let second_observed = pn_server(
            reported_proxy.id.clone(),
            "198.51.100.7".parse().unwrap(),
            57000,
        );

        heartbeat_with_observation(&selector, &reported_proxy, &first_observed).await;
        let first = selector.select(1).await.unwrap().unwrap();

        heartbeat_with_observation(&selector, &reported_proxy, &second_observed).await;
        let second = selector.select(1).await.unwrap().unwrap();

        assert_eq!(payload(&first).endpoints, payload(&second).endpoints);
        assert_eq!(
            payload(&second).endpoints,
            vec![PnServerEndpoint::new(
                "198.51.100.7".parse().unwrap(),
                443
            )]
        );
    }
}

#[cfg(test)]
#[path = "../tests/unit/pn_observed_address_recovery_tests.rs"]
mod pn_observed_address_recovery_tests;
