pub mod client;
pub mod control_channel;
pub mod errors;
pub mod pn_server_info;
pub mod proxy_node;
mod sequence;
pub mod server;
mod vpn_protocol;

pub use pn_server_info::*;
pub use proxy_node::*;
use serde::de::Visitor;
use serde::{Deserializer, Serializer, de};
pub use sfo_cmd_server as cmd_server;
use std::fmt;
pub use vpn_protocol::*;

pub fn serialize_u64_as_string<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub fn deserialize_u64_from_string<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct U64Visitor;

    impl<'de> Visitor<'de> for U64Visitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string representing a u64")
        }

        fn visit_str<E>(self, value: &str) -> Result<u64, E>
        where
            E: de::Error,
        {
            value.parse::<u64>().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_str(U64Visitor)
}
