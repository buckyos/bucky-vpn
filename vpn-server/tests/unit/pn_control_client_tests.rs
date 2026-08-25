use super::*;
use vpn_frame::{
    NodeTrafficReport, NodeTrafficReportResp, ProxyNodeHeartbeat, ProxyTrafficReport,
    ProxyTrafficReportResp, ValidatedPnConnection,
};

#[path = "pn_transport_mode_tests.rs"]
mod pn_transport_mode_tests;

struct FakeControlOps {
    network_id: Option<u64>,
}

#[async_trait::async_trait]
impl vpn_frame::control_channel::VpnControlClientOps for FakeControlOps {
    async fn report_pn_traffic_stats(
        &self,
        _reports: Vec<NodeTrafficReport>,
    ) -> vpn_frame::errors::VpnResult<Vec<NodeTrafficReportResp>> {
        Ok(Vec::new())
    }

    async fn report_proxy_heartbeat(
        &self,
        _heartbeat: ProxyNodeHeartbeat,
    ) -> vpn_frame::errors::VpnResult<()> {
        Ok(())
    }

    async fn report_proxy_traffic(
        &self,
        _reports: Vec<ProxyTrafficReport>,
    ) -> vpn_frame::errors::VpnResult<Vec<ProxyTrafficReportResp>> {
        Ok(Vec::new())
    }

    async fn validate_pn_connection(
        &self,
        _from: NodeId,
        _to: NodeId,
    ) -> vpn_frame::errors::VpnResult<Option<ValidatedPnConnection>> {
        Ok(self
            .network_id
            .map(|network_id| ValidatedPnConnection { network_id }))
    }
}

fn context() -> PnConnectionValidateContext {
    PnConnectionValidateContext {
        from: P2pId::from(vec![1u8; 32]),
        to: P2pId::from(vec![2u8; 32]),
        tunnel_id: 9u32.into(),
        kind: p2p_frame::pn::PnChannelKind::Stream,
        purpose: p2p_frame::networks::TunnelPurpose::from_value(&2000u16).unwrap(),
        is_control: false,
    }
}

#[tokio::test]
async fn remote_validator_accepts_selected_network_without_exporting_context() {
    let core = vpn_frame::control_channel::VpnCmdPnConnectionValidatorCore::new(Arc::new(
        FakeControlOps {
            network_id: Some(3),
        },
    ));
    let validator = VpnCmdPnConnectionValidator { core };
    let validation = validator.validate(&context()).await.unwrap();
    assert!(matches!(validation, ValidateResult::Accept));
}

#[tokio::test]
async fn remote_validator_rejects_when_control_returns_no_network() {
    let core = vpn_frame::control_channel::VpnCmdPnConnectionValidatorCore::new(Arc::new(
        FakeControlOps { network_id: None },
    ));
    let validator = VpnCmdPnConnectionValidator { core };
    let validation = validator.validate(&context()).await.unwrap();
    assert!(matches!(validation, ValidateResult::Reject(_)));
}
