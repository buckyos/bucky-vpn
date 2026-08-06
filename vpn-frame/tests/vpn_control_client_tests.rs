use async_trait::async_trait;
use bucky_raw_codec::{RawConvertTo, RawFrom};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vpn_frame::cmd_server::client::{CmdClient, CmdSend, SendGuard};
use vpn_frame::cmd_server::errors::{CmdErrorCode, CmdResult, cmd_err};
use vpn_frame::cmd_server::{CmdBody, CmdHandler, PeerId, TunnelId};
use vpn_frame::control_channel::VpnControlClientOps;
use vpn_frame::server::{NodeId, VpnControlClient, VpnControlCmdPkgLen};
use vpn_frame::{
    NodeTrafficReport, ProxyNodeHeartbeat, ProxyNodeHeartbeatId, ProxyTrafficReport,
    ReportPnTrafficStatsReq, ReportPnTrafficStatsResp, ReportProxyHeartbeatReq,
    ReportProxyHeartbeatResp, ReportProxyTrafficReq, ReportProxyTrafficResp, VPN_CMD_VERSION,
    ValidatePnConnectionReq, ValidatePnConnectionResp, VpnCmdCode,
};

struct FakeSend;

impl CmdSend<()> for FakeSend {
    fn get_tunnel_meta(&self) -> Option<Arc<()>> {
        None
    }

    fn get_remote_peer_id(&self) -> PeerId {
        Vec::<u8>::new().into()
    }
}

struct FakeGuard(FakeSend);

impl Deref for FakeGuard {
    type Target = FakeSend;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SendGuard<(), FakeSend> for FakeGuard {}

#[derive(Clone, Copy)]
enum ReplyMode {
    Response {
        result: u8,
        allowed: bool,
        network_id: Option<u64>,
    },
    TransportError,
    InvalidBody,
}

struct FakeCmdClient {
    mode: ReplyMode,
    calls: Mutex<Vec<(u8, u8, Duration)>>,
}

impl FakeCmdClient {
    fn success() -> Arc<Self> {
        Self::response(0, true, Some(42))
    }

    fn response(result: u8, allowed: bool, network_id: Option<u64>) -> Arc<Self> {
        Arc::new(Self {
            mode: ReplyMode::Response {
                result,
                allowed,
                network_id,
            },
            calls: Mutex::new(Vec::new()),
        })
    }

    fn with_mode(mode: ReplyMode) -> Arc<Self> {
        Arc::new(Self {
            mode,
            calls: Mutex::new(Vec::new()),
        })
    }

    fn response_body(&self, cmd: u8, body: &[u8]) -> CmdResult<CmdBody> {
        let ReplyMode::Response {
            result,
            allowed,
            network_id,
        } = self.mode
        else {
            unreachable!()
        };
        let data = match VpnCmdCode::try_from(cmd).unwrap() {
            VpnCmdCode::ReportPnTrafficStats => {
                let req = ReportPnTrafficStatsReq::clone_from_slice(body).unwrap();
                ReportPnTrafficStatsResp {
                    seq: req.seq,
                    result,
                    reports: Vec::new(),
                }
                .to_vec()
                .unwrap()
            }
            VpnCmdCode::ReportProxyHeartbeat => {
                let req = ReportProxyHeartbeatReq::clone_from_slice(body).unwrap();
                ReportProxyHeartbeatResp {
                    seq: req.seq,
                    result,
                }
                .to_vec()
                .unwrap()
            }
            VpnCmdCode::ReportProxyTraffic => {
                let req = ReportProxyTrafficReq::clone_from_slice(body).unwrap();
                ReportProxyTrafficResp {
                    seq: req.seq,
                    result,
                    reports: Vec::new(),
                }
                .to_vec()
                .unwrap()
            }
            VpnCmdCode::ValidatePnConnection => {
                let req = ValidatePnConnectionReq::clone_from_slice(body).unwrap();
                ValidatePnConnectionResp {
                    seq: req.seq,
                    result,
                    allowed,
                    network_id,
                }
                .to_vec()
                .unwrap()
            }
            other => panic!("unexpected VPN command: {other:?}"),
        };
        Ok(CmdBody::from_bytes(data))
    }
}

#[async_trait]
impl CmdClient<VpnControlCmdPkgLen, u8, (), FakeSend, FakeGuard> for FakeCmdClient {
    fn register_cmd_handler(&self, _cmd: u8, _handler: impl CmdHandler<VpnControlCmdPkgLen, u8>) {}

    async fn send(&self, _cmd: u8, _version: u8, _body: &[u8]) -> CmdResult<()> {
        unimplemented!()
    }

    async fn send_with_resp(
        &self,
        cmd: u8,
        version: u8,
        body: &[u8],
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.calls.lock().unwrap().push((cmd, version, timeout));
        match self.mode {
            ReplyMode::Response { .. } => self.response_body(cmd, body),
            ReplyMode::TransportError => Err(cmd_err!(CmdErrorCode::IoError, "mock send failed")),
            ReplyMode::InvalidBody => Ok(CmdBody::from_bytes(vec![0xff])),
        }
    }

    async fn send_parts(&self, _cmd: u8, _version: u8, _body: &[&[u8]]) -> CmdResult<()> {
        unimplemented!()
    }

    async fn send_parts_with_resp(
        &self,
        _cmd: u8,
        _version: u8,
        _body: &[&[u8]],
        _timeout: Duration,
    ) -> CmdResult<CmdBody> {
        unimplemented!()
    }

    async fn send_cmd(&self, _cmd: u8, _version: u8, _body: CmdBody) -> CmdResult<()> {
        unimplemented!()
    }

    async fn send_cmd_with_resp(
        &self,
        _cmd: u8,
        _version: u8,
        _body: CmdBody,
        _timeout: Duration,
    ) -> CmdResult<CmdBody> {
        unimplemented!()
    }

    async fn send_by_specify_tunnel(
        &self,
        _tunnel_id: TunnelId,
        _cmd: u8,
        _version: u8,
        _body: &[u8],
    ) -> CmdResult<()> {
        unimplemented!()
    }

    async fn send_by_specify_tunnel_with_resp(
        &self,
        _tunnel_id: TunnelId,
        _cmd: u8,
        _version: u8,
        _body: &[u8],
        _timeout: Duration,
    ) -> CmdResult<CmdBody> {
        unimplemented!()
    }

    async fn send_parts_by_specify_tunnel(
        &self,
        _tunnel_id: TunnelId,
        _cmd: u8,
        _version: u8,
        _body: &[&[u8]],
    ) -> CmdResult<()> {
        unimplemented!()
    }

    async fn send_parts_by_specify_tunnel_with_resp(
        &self,
        _tunnel_id: TunnelId,
        _cmd: u8,
        _version: u8,
        _body: &[&[u8]],
        _timeout: Duration,
    ) -> CmdResult<CmdBody> {
        unimplemented!()
    }

    async fn send_cmd_by_specify_tunnel(
        &self,
        _tunnel_id: TunnelId,
        _cmd: u8,
        _version: u8,
        _body: CmdBody,
    ) -> CmdResult<()> {
        unimplemented!()
    }

    async fn send_cmd_by_specify_tunnel_with_resp(
        &self,
        _tunnel_id: TunnelId,
        _cmd: u8,
        _version: u8,
        _body: CmdBody,
        _timeout: Duration,
    ) -> CmdResult<CmdBody> {
        unimplemented!()
    }

    async fn clear_all_tunnel(&self) {}

    async fn get_send(&self, _tunnel_id: TunnelId) -> CmdResult<FakeGuard> {
        unimplemented!()
    }
}

type TestControlClient = VpnControlClient<(), FakeSend, FakeGuard, FakeCmdClient>;

fn client(mock: Arc<FakeCmdClient>) -> Arc<TestControlClient> {
    VpnControlClient::new(mock, Duration::from_secs(9))
}

fn heartbeat() -> ProxyNodeHeartbeat {
    ProxyNodeHeartbeat {
        heartbeat_id: ProxyNodeHeartbeatId("heartbeat-1".to_owned()),
        pn_server: None,
    }
}

fn node_id(value: u8) -> NodeId {
    NodeId::from(&[value][..])
}

#[tokio::test]
async fn vpn_control_client_tests_forward_all_commands() {
    let mock = FakeCmdClient::success();
    let client = client(mock.clone());

    assert_eq!(
        client
            .report_pn_traffic_stats(Vec::<NodeTrafficReport>::new())
            .await
            .unwrap(),
        Vec::new()
    );
    client.report_proxy_heartbeat(heartbeat()).await.unwrap();
    assert_eq!(
        client
            .report_proxy_traffic(Vec::<ProxyTrafficReport>::new())
            .await
            .unwrap(),
        Vec::new()
    );
    assert_eq!(
        client
            .validate_pn_connection(node_id(1), node_id(2))
            .await
            .unwrap()
            .unwrap()
            .network_id,
        42
    );

    let calls = mock.calls.lock().unwrap();
    assert_eq!(
        calls.iter().map(|call| call.0).collect::<Vec<_>>(),
        vec![
            VpnCmdCode::ReportPnTrafficStats as u8,
            VpnCmdCode::ReportProxyHeartbeat as u8,
            VpnCmdCode::ReportProxyTraffic as u8,
            VpnCmdCode::ValidatePnConnection as u8,
        ]
    );
    assert!(calls.iter().all(|call| call.1 == VPN_CMD_VERSION));
    assert!(calls.iter().all(|call| call.2 == Duration::from_secs(9)));
}

#[tokio::test]
async fn vpn_control_client_tests_reject_non_zero_results() {
    let client = client(FakeCmdClient::response(7, true, Some(42)));

    assert!(
        client
            .report_pn_traffic_stats(Vec::<NodeTrafficReport>::new())
            .await
            .is_err()
    );
    assert!(client.report_proxy_heartbeat(heartbeat()).await.is_err());
    assert!(
        client
            .report_proxy_traffic(Vec::<ProxyTrafficReport>::new())
            .await
            .is_err()
    );
    assert!(
        client
            .validate_pn_connection(node_id(1), node_id(2))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn vpn_control_client_tests_preserve_validation_semantics() {
    let denied = client(FakeCmdClient::response(0, false, None));
    assert!(
        denied
            .validate_pn_connection(node_id(1), node_id(2))
            .await
            .unwrap()
            .is_none()
    );

    let malformed = client(FakeCmdClient::response(0, true, None));
    assert!(
        malformed
            .validate_pn_connection(node_id(1), node_id(2))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn vpn_control_client_tests_preserve_transport_and_decode_errors() {
    let transport = client(FakeCmdClient::with_mode(ReplyMode::TransportError));
    assert!(transport.report_proxy_heartbeat(heartbeat()).await.is_err());

    let invalid = client(FakeCmdClient::with_mode(ReplyMode::InvalidBody));
    assert!(invalid.report_proxy_heartbeat(heartbeat()).await.is_err());
}
