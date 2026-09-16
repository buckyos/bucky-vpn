#![allow(unused)]

use crate::api::Join;
use crate::setting::Setting;
use config::builder::DefaultState;
use sfo_http::http_server::HttpServerResult;
use sfo_http::http_util::HttpClient;
use std::path::{Path, PathBuf};
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use vpn_frame::server::NetworkGroupId;

const DEFAULT_LOCAL_API_IP: &str = "127.0.0.1";
const DEFAULT_LOCAL_API_PORT: u16 = 4536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalApiConfig {
    ip: String,
    port: u16,
}

impl LocalApiConfig {
    pub fn from_sources(
        env_config: &config::Config,
        file_config: &config::Config,
        setting: &Setting,
    ) -> Self {
        let ip = env_config
            .get_string("api.ip")
            .ok()
            .or_else(|| file_config.get_string("api.ip").ok())
            .or_else(|| setting.get::<String>("api.ip"))
            .unwrap_or_else(|| DEFAULT_LOCAL_API_IP.to_string());
        let port = env_config
            .get_int("api.port")
            .ok()
            .and_then(valid_port)
            .or_else(|| file_config.get_int("api.port").ok().and_then(valid_port))
            .or_else(|| setting.get::<u16>("api.port"))
            .unwrap_or(DEFAULT_LOCAL_API_PORT);
        Self { ip, port }
    }

    pub async fn load() -> VpnResult<Self> {
        let config = load_env_config();
        let data_dir = resolve_data_dir(&config);
        let setting_path = data_dir.join("setting.toml");
        let file_config = load_file_config(setting_path.as_path());
        let setting = Setting::load(setting_path.as_path()).await?;
        Ok(Self::from_sources(&config, &file_config, &setting))
    }

    pub fn bind_ip(&self) -> &str {
        self.ip.as_str()
    }

    pub fn bind_port(&self) -> u16 {
        self.port
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.ip, self.port)
    }
}

fn valid_port(port: i64) -> Option<u16> {
    u16::try_from(port).ok().filter(|port| *port > 0)
}

pub const DEFAULT_LISTEN_IP: &str = "0.0.0.0, ::";
const MAX_LISTEN_IPS: usize = 4;

/// Parse the client listen address configuration.
///
/// The value accepts one address or a comma/whitespace separated list so the
/// operator can listen on IPv4 only, IPv6 only, or both families. A wildcard
/// address is allowed at most once per family.
pub fn parse_listen_ips(value: &str) -> Result<Vec<std::net::IpAddr>, String> {
    let mut ips: Vec<std::net::IpAddr> = Vec::new();
    for raw in value.split([',', ' ', '\t', '\r', '\n']) {
        let entry = raw.trim();
        if entry.is_empty() {
            continue;
        }
        let ip = entry
            .parse::<std::net::IpAddr>()
            .map_err(|err| format!("p2p.listen_ip entry {entry:?} is not an IP address: {err}"))?;
        if ips.contains(&ip) {
            return Err(format!("p2p.listen_ip contains duplicate address {ip}"));
        }
        ips.push(ip);
    }

    if ips.is_empty() {
        return Err("p2p.listen_ip must contain at least one IP address".to_owned());
    }
    if ips.len() > MAX_LISTEN_IPS {
        return Err(format!(
            "p2p.listen_ip accepts at most {MAX_LISTEN_IPS} addresses"
        ));
    }

    let wildcard_v4 = ips
        .iter()
        .filter(|ip| ip.is_ipv4() && ip.is_unspecified())
        .count();
    let wildcard_v6 = ips
        .iter()
        .filter(|ip| ip.is_ipv6() && ip.is_unspecified())
        .count();
    if wildcard_v4 > 1 || wildcard_v6 > 1 {
        return Err("p2p.listen_ip accepts at most one wildcard address per family".to_owned());
    }

    Ok(ips)
}

pub fn load_env_config() -> config::Config {
    config::ConfigBuilder::<DefaultState>::default()
        .add_source(config::Environment::with_prefix("VPN").separator("_"))
        .build()
        .unwrap()
}

pub fn load_file_config(config_file: &Path) -> config::Config {
    config::ConfigBuilder::<DefaultState>::default()
        .add_source(config::File::from(config_file).required(false))
        .build()
        .unwrap()
}

pub fn resolve_data_dir(config: &config::Config) -> PathBuf {
    match config.get_string("data.dir") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            #[cfg(target_os = "windows")]
            {
                std::env::current_exe()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("data")
            }
            #[cfg(target_os = "macos")]
            {
                PathBuf::from("/Library/Application Support/BuckyVPN")
            }
            #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
            {
                PathBuf::from("/var/bucky_vpn")
            }
        }
    }
}

pub struct Cli;

impl Cli {
    pub async fn join(
        server: String,
        server_port: u16,
        server_id: String,
        server_name: Option<String>,
        group_id: NetworkGroupId,
        name: Option<String>,
    ) -> VpnResult<()> {
        let local_api = LocalApiConfig::load().await?;
        let http_client = HttpClient::new(5, Some(local_api.base_url().as_str()));
        let result: HttpServerResult<()> = http_client
            .post_json(
                "/join",
                &Join {
                    server,
                    server_port,
                    server_id,
                    server_name,
                    group_id,
                    name,
                },
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        if result.err != 0 {
            Err(vpn_err!(
                VpnErrorCode::Failed,
                "err: {}, msg: {}",
                result.err,
                result.msg
            ))
        } else {
            Ok(())
        }
    }

    pub async fn get_state(_server: String) -> VpnResult<()> {
        let local_api = LocalApiConfig::load().await?;
        let http_client = HttpClient::new(5, Some(local_api.base_url().as_str()));
        let result: HttpServerResult<()> = http_client
            .get_json("/state")
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        if result.err != 0 {
            Err(vpn_err!(
                VpnErrorCode::Failed,
                "err: {}, msg: {}",
                result.err,
                result.msg
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn empty_config() -> config::Config {
        config::ConfigBuilder::<DefaultState>::default()
            .build()
            .unwrap()
    }

    fn override_config(ip: &str, port: u16) -> config::Config {
        config::ConfigBuilder::<DefaultState>::default()
            .set_override("api.ip", ip)
            .unwrap()
            .set_override("api.port", port)
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn listen_ip_defaults_to_ipv4_and_ipv6_wildcards() {
        let ips = parse_listen_ips(DEFAULT_LISTEN_IP).unwrap();

        assert_eq!(
            ips,
            vec![
                std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED)
            ]
        );
    }

    #[test]
    fn listen_ip_accepts_ipv6_and_mixed_lists_in_order() {
        let ips = parse_listen_ips("0.0.0.0, ::").unwrap();

        assert_eq!(
            ips,
            vec![
                std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED)
            ]
        );
        assert_eq!(parse_listen_ips("::1").unwrap().len(), 1);
        assert!(parse_listen_ips("[::]").is_err());
    }

    #[test]
    fn listen_ip_rejects_invalid_and_ambiguous_inputs() {
        assert!(parse_listen_ips("").is_err());
        assert!(parse_listen_ips("not-an-ip").is_err());
        assert!(parse_listen_ips("localhost").is_err());
        assert!(parse_listen_ips("0.0.0.0,0.0.0.0").is_err());
        assert!(parse_listen_ips("::,::").is_err());
        assert!(parse_listen_ips("0.0.0.0,0.0.0.0,0.0.0.0").is_err());
        assert!(parse_listen_ips("0.0.0.0,::,::1,::2,::3").is_err());
    }

    fn temp_setting_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bucky-vpn-local-api-config-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.join("setting.toml")
    }

    #[tokio::test]
    async fn local_api_config_uses_default_address() {
        let path = temp_setting_path();
        let setting = Setting::load(path.as_path()).await.unwrap();
        let file_config = load_file_config(path.as_path());

        let local_api = LocalApiConfig::from_sources(&empty_config(), &file_config, &setting);

        assert_eq!(local_api.bind_ip(), "127.0.0.1");
        assert_eq!(local_api.bind_port(), 4536);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn local_api_config_reads_setting_file_api_table() {
        let path = temp_setting_path();
        fs::write(path.as_path(), "[api]\nip = \"127.0.0.2\"\nport = 4540\n").unwrap();
        let setting = Setting::load(path.as_path()).await.unwrap();
        let file_config = load_file_config(path.as_path());

        let local_api = LocalApiConfig::from_sources(&empty_config(), &file_config, &setting);

        assert_eq!(local_api.bind_ip(), "127.0.0.2");
        assert_eq!(local_api.bind_port(), 4540);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn local_api_config_env_overrides_setting_file() {
        let path = temp_setting_path();
        fs::write(path.as_path(), "[api]\nip = \"127.0.0.2\"\nport = 4540\n").unwrap();
        let setting = Setting::load(path.as_path()).await.unwrap();
        let file_config = load_file_config(path.as_path());

        let local_api = LocalApiConfig::from_sources(
            &override_config("127.0.0.3", 4541),
            &file_config,
            &setting,
        );

        assert_eq!(local_api.bind_ip(), "127.0.0.3");
        assert_eq!(local_api.bind_port(), 4541);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
