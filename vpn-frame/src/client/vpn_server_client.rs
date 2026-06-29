use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use crate::sequence::{Sequence, SequenceGenerator};
use crate::server::{NetworkGroupId, NetworkId, NodeId};
use crate::{
    GetVpnInfoReq, GetVpnInfoResp, JoinNetworkGroupReq, JoinNetworkGroupResp, NodeVpnInfo,
    PnServerInfo, QueryNodeReq, QueryNodeResp, ReportPnTrafficStatsReq, ReportPnTrafficStatsResp,
    ValidatePnConnectionReq, ValidatePnConnectionResp, VpnCmdCode, VpnCmdHeader, VpnTunnelId,
};
use bucky_raw_codec::{RawConvertTo, RawFrom};
use callback_result::CallbackWaiter;
use sfo_cmd_server::client::{CmdClient, CmdSend, SendGuard};
use sfo_cmd_server::errors::{CmdErrorCode, into_cmd_err};
use sfo_cmd_server::{CmdBody, CmdTunnelMeta, PeerId};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct VpnServerClient<
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<u16, u8, M, S, G>,
> {
    cmd_client: Arc<T>,
    version: u8,
    conn_timeout: Duration,
    gen_seq: Arc<SequenceGenerator>,
    join_resp_waiter: CallbackWaiter<Sequence, VpnResult<JoinNetworkGroupResp>>,
    get_vpn_info_resp_waiter: CallbackWaiter<Sequence, VpnResult<GetVpnInfoResp>>,
    query_node_resp_waiter: CallbackWaiter<Sequence, VpnResult<QueryNodeResp>>,
    report_pn_traffic_resp_waiter: CallbackWaiter<Sequence, VpnResult<ReportPnTrafficStatsResp>>,
    validate_pn_connection_resp_waiter:
        CallbackWaiter<Sequence, VpnResult<ValidatePnConnectionResp>>,
    _p: std::marker::PhantomData<Arc<Mutex<(M, S, G)>>>,
}

impl<M: CmdTunnelMeta, S: CmdSend<M>, G: SendGuard<M, S>, T: CmdClient<u16, u8, M, S, G>>
    VpnServerClient<M, S, G, T>
{
    pub fn new(cmd_client: Arc<T>, conn_timeout: Duration) -> Arc<Self> {
        let this = Arc::new(Self {
            cmd_client,
            version: 0,
            conn_timeout,
            gen_seq: Arc::new(SequenceGenerator::new()),
            join_resp_waiter: CallbackWaiter::new(),
            get_vpn_info_resp_waiter: CallbackWaiter::new(),
            query_node_resp_waiter: CallbackWaiter::new(),
            report_pn_traffic_resp_waiter: CallbackWaiter::new(),
            validate_pn_connection_resp_waiter: CallbackWaiter::new(),
            _p: Default::default(),
        });
        this.register_cmd_handler();
        this
    }

    fn register_cmd_handler(self: &Arc<Self>) {
        let this = self.clone();
        self.cmd_client.register_cmd_handler(
            VpnCmdCode::JoinNetworkGroupResp as u8,
            move |_local_id: PeerId,
                  _peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let resp = JoinNetworkGroupResp::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let _ = this.join_resp_waiter.set_result(resp.seq, Ok(resp));
                    Ok(None)
                }
            },
        );

        let this = self.clone();
        self.cmd_client.register_cmd_handler(
            VpnCmdCode::GetVpnInfoResp as u8,
            move |_local_id: PeerId,
                  _peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let resp = GetVpnInfoResp::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let _ = this.get_vpn_info_resp_waiter.set_result(resp.seq, Ok(resp));
                    Ok(None)
                }
            },
        );

        let this = self.clone();
        self.cmd_client.register_cmd_handler(
            VpnCmdCode::QueryNodeResp as u8,
            move |_local_id: PeerId,
                  _peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let resp = QueryNodeResp::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    log::info!("recv query node resp: {:?}", resp.seq);
                    let _ = this.query_node_resp_waiter.set_result(resp.seq, Ok(resp));
                    Ok(None)
                }
            },
        );

        let this = self.clone();
        self.cmd_client.register_cmd_handler(
            VpnCmdCode::ReportPnTrafficStatsResp as u8,
            move |_local_id: PeerId,
                  _peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let resp = ReportPnTrafficStatsResp::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let _ = this
                        .report_pn_traffic_resp_waiter
                        .set_result(resp.seq, Ok(resp));
                    Ok(None)
                }
            },
        );

        let this = self.clone();
        self.cmd_client.register_cmd_handler(
            VpnCmdCode::ValidatePnConnectionResp as u8,
            move |_local_id: PeerId,
                  _peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let resp = ValidatePnConnectionResp::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let _ = this
                        .validate_pn_connection_resp_waiter
                        .set_result(resp.seq, Ok(resp));
                    Ok(None)
                }
            },
        );
    }

    pub async fn join_network_group(
        &self,
        network_group_id: NetworkGroupId,
        name: Option<String>,
    ) -> VpnResult<()> {
        let req = JoinNetworkGroupReq {
            seq: self.gen_seq.generate(),
            name,
            group_id: network_group_id,
        };
        let future = self
            .join_resp_waiter
            .create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client
            .send(
                VpnCmdCode::JoinNetworkGroup as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(())
        }
    }

    pub async fn get_vpn_info(
        &self,
        cur_version: Option<u16>,
        client_version: Option<String>,
    ) -> VpnResult<(u16, Vec<NodeVpnInfo>)> {
        let req = GetVpnInfoReq {
            seq: self.gen_seq.generate(),
            info_version: cur_version,
            client_version,
        };
        let future = self
            .get_vpn_info_resp_waiter
            .create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client
            .send(
                VpnCmdCode::GetVpnInfo as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok((ret.info_version, ret.vpn_list))
        }
    }

    pub async fn query_node(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        ip: IpAddr,
    ) -> VpnResult<Option<NodeId>> {
        let req = QueryNodeReq {
            seq: self.gen_seq.generate(),
            group_id: network_group_id,
            network_id,
            ip,
        };

        let future = self
            .query_node_resp_waiter
            .create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client
            .send(
                VpnCmdCode::QueryNode as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        Ok(ret.node_id)
    }

    pub async fn report_pn_traffic_stats(
        &self,
        node_id: NodeId,
        pn_server: Option<PnServerInfo>,
        tx_bytes: u64,
        rx_bytes: u64,
    ) -> VpnResult<()> {
        let req = ReportPnTrafficStatsReq {
            seq: self.gen_seq.generate(),
            node_id,
            pn_server,
            tx_bytes,
            rx_bytes,
        };
        let future = self
            .report_pn_traffic_resp_waiter
            .create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client
            .send(
                VpnCmdCode::ReportPnTrafficStats as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(())
        }
    }

    pub async fn validate_pn_connection(&self, from: NodeId, to: NodeId) -> VpnResult<bool> {
        let req = ValidatePnConnectionReq {
            seq: self.gen_seq.generate(),
            from,
            to,
        };
        let future = self
            .validate_pn_connection_resp_waiter
            .create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client
            .send(
                VpnCmdCode::ValidatePnConnection as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(ret.allowed)
        }
    }
}
