use vpn_frame::client::VpnServerClient;
use vpn_frame::cmd_server::CmdTunnelMeta;
use vpn_frame::cmd_server::client::{CmdClient, CmdSend, SendGuard};
use vpn_frame::server::NodeId;
use vpn_frame::{
    NodeTrafficReport, ProxyNodeHeartbeat, ProxyTrafficReport, VpnCmdPkgLen,
};

async fn removed_report_pn_traffic_stats<M, S, G, T>(
    client: &VpnServerClient<M, S, G, T>,
    reports: Vec<NodeTrafficReport>,
) where
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnCmdPkgLen, u8, M, S, G>,
{
    let _ = VpnServerClient::report_pn_traffic_stats(client, reports).await;
}

async fn removed_report_proxy_heartbeat<M, S, G, T>(
    client: &VpnServerClient<M, S, G, T>,
    heartbeat: ProxyNodeHeartbeat,
) where
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnCmdPkgLen, u8, M, S, G>,
{
    let _ = VpnServerClient::report_proxy_heartbeat(client, heartbeat).await;
}

async fn removed_report_proxy_traffic<M, S, G, T>(
    client: &VpnServerClient<M, S, G, T>,
    reports: Vec<ProxyTrafficReport>,
) where
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnCmdPkgLen, u8, M, S, G>,
{
    let _ = VpnServerClient::report_proxy_traffic(client, reports).await;
}

async fn removed_validate_pn_connection<M, S, G, T>(
    client: &VpnServerClient<M, S, G, T>,
    from: NodeId,
    to: NodeId,
) where
    M: CmdTunnelMeta,
    S: CmdSend<M>,
    G: SendGuard<M, S>,
    T: CmdClient<VpnCmdPkgLen, u8, M, S, G>,
{
    let _ = VpnServerClient::validate_pn_connection(client, from, to).await;
}

fn main() {}
