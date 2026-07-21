use crate::control_channel::VpnControlClientOps;
use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use crate::sequence::SequenceGenerator;
use crate::server::NodeId;
use crate::{
    NodeTrafficReport, NodeTrafficReportResp, ProxyNodeHeartbeat, ProxyTrafficReport,
    ProxyTrafficReportResp, ReportPnTrafficStatsReq, ReportPnTrafficStatsResp,
    ReportProxyHeartbeatReq, ReportProxyHeartbeatResp, ReportProxyTrafficReq,
    ReportProxyTrafficResp, VPN_CMD_VERSION, ValidatePnConnectionReq, ValidatePnConnectionResp,
    ValidatedPnConnection, VpnCmdCode,
};
use async_trait::async_trait;
use bucky_raw_codec::{RawConvertTo, RawFrom};
use sfo_cmd_server::CmdTunnelMeta;
use sfo_cmd_server::client::{CmdClient, CmdSend, SendGuard};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const VPN_CONTROL_CMD_MAX_BYTES: usize = 10 * 1024 * 1024;
pub type VpnControlCmdPkgLen = sfo_cmd_server::U24<{ VPN_CONTROL_CMD_MAX_BYTES as u64 }>;
pub type VpnControlCmdHeader = sfo_cmd_server::CmdHeader<VpnControlCmdPkgLen, u8>;

pub struct VpnControlClient<
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnControlCmdPkgLen, u8, M, S, G>,
> {
    cmd_client: Arc<T>,
    version: u8,
    conn_timeout: Duration,
    gen_seq: Arc<SequenceGenerator>,
    _p: std::marker::PhantomData<Arc<Mutex<(M, S, G)>>>,
}

impl<
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnControlCmdPkgLen, u8, M, S, G>,
> VpnControlClient<M, S, G, T>
{
    pub fn new(cmd_client: Arc<T>, conn_timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            cmd_client,
            version: VPN_CMD_VERSION,
            conn_timeout,
            gen_seq: Arc::new(SequenceGenerator::new()),
            _p: Default::default(),
        })
    }
}

#[async_trait]
impl<M, S, G, T> VpnControlClientOps for VpnControlClient<M, S, G, T>
where
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnControlCmdPkgLen, u8, M, S, G>,
{
    async fn report_pn_traffic_stats(
        &self,
        reports: Vec<NodeTrafficReport>,
    ) -> VpnResult<Vec<NodeTrafficReportResp>> {
        let req = ReportPnTrafficStatsReq {
            seq: self.gen_seq.generate(),
            reports,
        };
        let mut body = self
            .cmd_client
            .send_with_resp(
                VpnCmdCode::ReportPnTrafficStats as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
                self.conn_timeout,
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let data = body
            .read_all()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let ret = ReportPnTrafficStatsResp::clone_from_slice(data.as_slice())
            .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(ret.reports)
        }
    }

    async fn report_proxy_heartbeat(&self, heartbeat: ProxyNodeHeartbeat) -> VpnResult<()> {
        let req = ReportProxyHeartbeatReq {
            seq: self.gen_seq.generate(),
            heartbeat,
        };
        let mut body = self
            .cmd_client
            .send_with_resp(
                VpnCmdCode::ReportProxyHeartbeat as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
                self.conn_timeout,
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let data = body
            .read_all()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let ret = ReportProxyHeartbeatResp::clone_from_slice(data.as_slice())
            .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(())
        }
    }

    async fn report_proxy_traffic(
        &self,
        reports: Vec<ProxyTrafficReport>,
    ) -> VpnResult<Vec<ProxyTrafficReportResp>> {
        let req = ReportProxyTrafficReq {
            seq: self.gen_seq.generate(),
            reports,
        };
        let mut body = self
            .cmd_client
            .send_with_resp(
                VpnCmdCode::ReportProxyTraffic as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
                self.conn_timeout,
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let data = body
            .read_all()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let ret = ReportProxyTrafficResp::clone_from_slice(data.as_slice())
            .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(ret.reports)
        }
    }

    async fn validate_pn_connection(
        &self,
        from: NodeId,
        to: NodeId,
    ) -> VpnResult<Option<ValidatedPnConnection>> {
        let req = ValidatePnConnectionReq {
            seq: self.gen_seq.generate(),
            from,
            to,
        };
        let mut body = self
            .cmd_client
            .send_with_resp(
                VpnCmdCode::ValidatePnConnection as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
                self.conn_timeout,
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let data = body
            .read_all()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let ret = ValidatePnConnectionResp::clone_from_slice(data.as_slice())
            .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            ret.validated_connection()
        }
    }
}
