use num_derive::{FromPrimitive, ToPrimitive};
pub use sfo_result::err as vpn_err;
pub use sfo_result::into_err as into_vpn_err;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, FromPrimitive, ToPrimitive)]
pub enum VpnErrorCode {
    Ok = 0,
    #[default]
    Failed,
    InvalidParam,
    IoError,
    RawCodecError,
    RsaError,
    Timeout,
    NetworkGroupNotExist,
    NotFoundNode,
    NoPermission,
    InvalidIp,
}

impl Into<u16> for VpnErrorCode {
    fn into(self) -> u16 {
        self as u16
    }
}
pub type VpnResult<T> = sfo_result::Result<T, VpnErrorCode>;
pub type VpnError = sfo_result::Error<VpnErrorCode>;
