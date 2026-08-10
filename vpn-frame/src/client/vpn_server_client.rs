use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use crate::sequence::SequenceGenerator;
use crate::server::{NetworkGroupId, NetworkId, NodeId};
use crate::{
    GetVpnInfoReq, GetVpnInfoResp, JoinNetworkGroupReq, JoinNetworkGroupResp, NodeVpnInfo,
    QueryNodeReq, QueryNodeResp, VPN_CMD_VERSION, VpnCmdCode, VpnCmdPkgLen,
};
use bucky_raw_codec::{RawConvertTo, RawFrom};
use sfo_cmd_server::CmdTunnelMeta;
use sfo_cmd_server::client::{CmdClient, CmdSend, SendGuard};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct VpnServerClient<
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnCmdPkgLen, u8, M, S, G>,
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
    T: CmdClient<VpnCmdPkgLen, u8, M, S, G>,
>
    VpnServerClient<M, S, G, T>
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
        let mut body = self
            .cmd_client
            .send_with_resp(
                VpnCmdCode::JoinNetworkGroup as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
                self.conn_timeout,
            )
            .await
            .map_err(|err| {
                vpn_err!(
                    VpnErrorCode::IoError,
                    "vpn command version {} rejected by server: {}",
                    self.version,
                    err
                )
            })?;

        let data = body
            .read_all()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let ret = JoinNetworkGroupResp::clone_from_slice(data.as_slice())
            .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(())
        }
    }

    pub async fn get_vpn_info(
        &self,
        cur_version: Option<u16>,
        cur_pn_info_version: Option<u32>,
        client_version: Option<String>,
    ) -> VpnResult<((u16, u32), Vec<NodeVpnInfo>)> {
        let req = GetVpnInfoReq {
            seq: self.gen_seq.generate(),
            info_version: cur_version,
            pn_info_version: cur_pn_info_version,
            client_version,
        };
        let mut body = self
            .cmd_client
            .send_with_resp(
                VpnCmdCode::GetVpnInfo as u8,
                self.version,
                req.to_vec()
                    .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?
                    .as_slice(),
                self.conn_timeout,
            )
            .await
            .map_err(|err| {
                vpn_err!(
                    VpnErrorCode::IoError,
                    "vpn command version {} rejected by server: {}",
                    self.version,
                    err
                )
            })?;

        let data = body
            .read_all()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let ret = GetVpnInfoResp::clone_from_slice(data.as_slice())
            .map_err(|err| {
                vpn_err!(
                    VpnErrorCode::RawCodecError,
                    "vpn command version {} response is incompatible: {}",
                    self.version,
                    err
                )
            })?;
        if ret.result != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "result = {}", ret.result))
        } else {
            Ok(((ret.info_version, ret.pn_info_version), ret.vpn_list))
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

        let mut body = self
            .cmd_client
            .send_with_resp(
                VpnCmdCode::QueryNode as u8,
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
        let ret = QueryNodeResp::clone_from_slice(data.as_slice())
            .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        Ok(ret.node_id)
    }

}
