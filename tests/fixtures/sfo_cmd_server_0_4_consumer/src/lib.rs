use vpn_frame::{VpnCmdHeader, VpnCmdPkgLen};

pub use vpn_frame::control_channel::VpnControlClientOps;
pub use vpn_frame::server::VpnControlClient;

pub fn package_length_value(value: VpnCmdPkgLen) -> u16 {
    value.into()
}

pub fn preserve_header_type(header: VpnCmdHeader) -> VpnCmdHeader {
    header
}
