use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use bucky_raw_codec::{RawConvertTo, RawFrom};
use callback_result::CallbackWaiter;
use num_traits::FromPrimitive;
use sfo_cmd_server::client::CmdClient;
use sfo_cmd_server::{CmdBodyRead, PeerId};
use sfo_cmd_server::errors::{into_cmd_err, CmdErrorCode};
use crate::{GetVpnInfoReq, GetVpnInfoResp, JoinNetworkGroupReq, JoinNetworkGroupResp, NodeVpnInfo, QueryNodeReq, QueryNodeResp, VpnTunnelId, VpnCmdCode, VpnCmdHeader};
use crate::errors::{into_vpn_err, vpn_err, VpnErrorCode, VpnResult};
use crate::sequence::{Sequence, SequenceGenerator};
use crate::server::{NetworkGroupId, NetworkId, NodeId};

pub struct VpnServerClient<T: CmdClient<u16, u8>> {
    cmd_client: Arc<T>,
    version: u8,
    conn_timeout: Duration,
    gen_seq: Arc<SequenceGenerator>,
    join_resp_waiter: CallbackWaiter<Sequence, VpnResult<JoinNetworkGroupResp>>,
    get_vpn_info_resp_waiter: CallbackWaiter<Sequence, VpnResult<GetVpnInfoResp>>,
    query_node_resp_waiter: CallbackWaiter<Sequence, VpnResult<QueryNodeResp>>,
}

impl<T: CmdClient<u16, u8>> VpnServerClient<T> {
    pub fn new(cmd_client: Arc<T>, conn_timeout: Duration) -> Arc<Self> {
        let this = Arc::new(Self {
            cmd_client,
            version: 0,
            conn_timeout,
            gen_seq: Arc::new(SequenceGenerator::new()),
            join_resp_waiter: CallbackWaiter::new(),
            get_vpn_info_resp_waiter: CallbackWaiter::new(),
            query_node_resp_waiter: CallbackWaiter::new(),
        });
        this.register_cmd_handler();
        this
    }

    fn register_cmd_handler(self: &Arc<Self>) {
        let this = self.clone();
        self.cmd_client.register_cmd_handler(VpnCmdCode::JoinNetworkGroupResp as u8, move |_peer_id: PeerId, _tunnel_id: VpnTunnelId, _header: VpnCmdHeader, mut body: CmdBodyRead| {
            let this = this.clone();
            async move {
                let data = body.read_all().await?;
                let resp = JoinNetworkGroupResp::clone_from_slice(data.as_slice()).map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                let _ = this.join_resp_waiter.set_result(resp.seq, Ok(resp));
                Ok(())
            }
        });

        let this = self.clone();
        self.cmd_client.register_cmd_handler(VpnCmdCode::GetVpnInfoResp as u8, move |_peer_id: PeerId, _tunnel_id: VpnTunnelId, _header: VpnCmdHeader, mut body: CmdBodyRead| {
            let this = this.clone();
            async move {
                let data = body.read_all().await?;
                let resp = GetVpnInfoResp::clone_from_slice(data.as_slice()).map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                let _ = this.get_vpn_info_resp_waiter.set_result(resp.seq, Ok(resp));
                Ok(())
            }
        });

        let this = self.clone();
        self.cmd_client.register_cmd_handler(VpnCmdCode::QueryNodeResp as u8, move |_peer_id: PeerId, _tunnel_id: VpnTunnelId, _header: VpnCmdHeader, mut body: CmdBodyRead| {
            let this = this.clone();
            async move {
                let data = body.read_all().await?;
                let resp = QueryNodeResp::clone_from_slice(data.as_slice()).map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                log::info!("recv query node resp: {:?}", resp.seq);
                let _ = this.query_node_resp_waiter.set_result(resp.seq, Ok(resp));
                Ok(())
            }
        });
    }



    pub async fn join_network_group(&self, network_group_id: NetworkGroupId, name: Option<String>) -> VpnResult<()> {
        let req = JoinNetworkGroupReq {
            seq: self.gen_seq.generate(),
            name,
            group_id: network_group_id,
        };
        let future = self.join_resp_waiter.create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client.send(VpnCmdCode::JoinNetworkGroup as u8,
                             self.version,
                             req.to_vec().map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?.as_slice()).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future.await.map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::from_u8(ret.result).unwrap_or(VpnErrorCode::Failed)))
        } else {
            Ok(())
        }
    }

    pub async fn get_vpn_info(&self, cur_version: u64, client_version: String) -> VpnResult<(u64, Vec<NodeVpnInfo>)> {
        let req = GetVpnInfoReq {
            seq: self.gen_seq.generate(),
            info_version: cur_version,
            client_version,
        };
        let future = self.get_vpn_info_resp_waiter.create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client.send(VpnCmdCode::GetVpnInfo as u8, self.version, req.to_vec().map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?.as_slice()).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future.await.map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::from_u8(ret.result).unwrap_or(VpnErrorCode::Failed)))
        } else {
            Ok((ret.info_version, ret.vpn_list))
        }
    }

    pub async fn query_node(&self, network_group_id: NetworkGroupId, network_id: NetworkId, ip: IpAddr) -> VpnResult<Option<NodeId>> {
        let req = QueryNodeReq {
            seq: self.gen_seq.generate(),
            group_id: network_group_id,
            network_id,
            ip,
        };

        let future = self.query_node_resp_waiter.create_timeout_result_future(req.seq, self.conn_timeout)
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.cmd_client.send(VpnCmdCode::QueryNode as u8, self.version, req.to_vec().map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?.as_slice()).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let ret = future.await.map_err(into_vpn_err!(VpnErrorCode::Timeout))??;
        Ok(ret.node_id)
    }
}


