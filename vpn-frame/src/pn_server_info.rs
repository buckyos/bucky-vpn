use crate::PnServerInfo;
use crate::errors::{VpnErrorCode, VpnResult, vpn_err};
use bucky_raw_codec::{RawDecode, RawEncode};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

fn default_endpoint_protocol() -> String {
    PnServerEndpoint::PROTOCOL_QUIC.to_string()
}

#[derive(
    Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, PartialOrd, Ord, RawEncode, RawDecode,
)]
pub struct PnServerEndpoint {
    #[serde(default = "default_endpoint_protocol")]
    pub protocol: String,
    pub ip: IpAddr,
    pub port: u16,
}

pub type PnServerAddress = PnServerEndpoint;

impl PnServerEndpoint {
    pub const PROTOCOL_QUIC: &'static str = "quic";
    pub const PROTOCOL_TCP: &'static str = "tcp";

    pub fn new(ip: IpAddr, port: u16) -> Self {
        Self::new_with_protocol(Self::PROTOCOL_QUIC, ip, port)
    }

    pub fn new_tcp(ip: IpAddr, port: u16) -> Self {
        Self::new_with_protocol(Self::PROTOCOL_TCP, ip, port)
    }

    pub fn new_with_protocol(protocol: impl Into<String>, ip: IpAddr, port: u16) -> Self {
        Self {
            protocol: protocol.into(),
            ip,
            port,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, Default)]
pub struct PnServerPortMapping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quic: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp: Option<u16>,
}

impl PnServerPortMapping {
    pub fn is_empty(&self) -> bool {
        self.quic.is_none() && self.tcp.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, Default)]
pub struct PnServerInfoPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertised_ip: Option<IpAddr>,
    #[serde(default)]
    pub endpoints: Vec<PnServerEndpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_mapping: Option<PnServerPortMapping>,
}

pub type ReportedPnServerInfoPayload = PnServerInfoPayload;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, Default)]
pub struct ClientPnServerInfoPayload {
    pub proxy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub endpoints: Vec<PnServerEndpoint>,
}

impl PnServerInfoPayload {
    pub fn new_with_endpoint(endpoint: PnServerEndpoint) -> Self {
        Self::new_with_endpoints(vec![endpoint])
    }

    pub fn new_with_primary_address(
        primary: PnServerEndpoint,
        addresses: Vec<PnServerEndpoint>,
    ) -> Self {
        let mut endpoints = Vec::with_capacity(addresses.len() + 1);
        endpoints.push(primary);
        endpoints.extend(addresses);
        Self::new_with_endpoints(endpoints)
    }

    pub fn new_with_endpoints(endpoints: Vec<PnServerEndpoint>) -> Self {
        let mut info = Self::default();
        for endpoint in endpoints {
            info.add_endpoint(endpoint);
        }
        info
    }

    pub fn add_endpoint(&mut self, endpoint: PnServerEndpoint) {
        if !self.endpoints.contains(&endpoint) {
            self.endpoints.push(endpoint);
        }
    }

    pub fn primary_endpoint(&self) -> Option<&PnServerEndpoint> {
        self.endpoints.first()
    }

    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = normalize_name(name);
        self
    }

    pub fn with_advertised_ip(mut self, advertised_ip: Option<IpAddr>) -> Self {
        self.advertised_ip = advertised_ip;
        self
    }

    pub fn with_port_mapping(mut self, port_mapping: Option<PnServerPortMapping>) -> Self {
        self.port_mapping = port_mapping.filter(|mapping| !mapping.is_empty());
        self
    }

    pub fn remote_name<'a>(&'a self, fallback_id: &'a str) -> &'a str {
        self.name.as_deref().unwrap_or(fallback_id)
    }
}

pub fn normalize_name(name: Option<String>) -> Option<String> {
    name.and_then(|name| {
        let name = name.trim().to_owned();
        if name.is_empty() { None } else { Some(name) }
    })
}

pub fn encode_pn_server_info(
    id: impl Into<String>,
    payload: PnServerInfoPayload,
) -> VpnResult<PnServerInfo> {
    let info = serde_json::to_vec(&payload).map_err(|err| {
        vpn_err!(
            VpnErrorCode::RawCodecError,
            "encode pn server info failed: {}",
            err
        )
    })?;
    Ok(PnServerInfo::new(id.into(), info))
}

pub fn decode_pn_server_info(pn_server: &PnServerInfo) -> VpnResult<PnServerInfoPayload> {
    if pn_server.info.is_empty() {
        return Ok(PnServerInfoPayload::default());
    }
    serde_json::from_slice(&pn_server.info).map_err(|err| {
        vpn_err!(
            VpnErrorCode::RawCodecError,
            "decode pn server info {} failed: {}",
            pn_server.id,
            err
        )
    })
}

pub fn with_pn_server_name(
    pn_server: PnServerInfo,
    name: Option<String>,
) -> VpnResult<PnServerInfo> {
    let mut payload = decode_pn_server_info(&pn_server)?;
    payload.name = normalize_name(name);
    encode_pn_server_info(pn_server.id, payload)
}
