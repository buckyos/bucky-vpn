mod api;
mod cli;
mod p2p_vpn;
mod setting;
use std::fs::create_dir_all;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use async_trait::async_trait;
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::stack::{init_p2p, P2pConfig};
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use sfo_http::http_server::HttpServerConfig;
use sfo_http::tide_server::TideHttpServer;
use vpn_frame::client::{PacketRecv};
use vpn_frame::errors::VpnResult;
use crate::api::Api;
use crate::cli::Cli;
use crate::p2p_vpn::{init_p2p_vpn_client_manager, vpn_client_manager, JoinRecord};
use crate::setting::Setting;

struct TestRecv {

}

impl TestRecv {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl PacketRecv for TestRecv {
    async fn on_recv(&self, target: IpAddr, packet: &[u8]) -> VpnResult<()> {
        Ok(())
    }

}


#[tokio::main]
async fn main() {
    sfo_log::Logger::new("vpn-client")
        .set_log_to_file(true)
        .set_log_file_count(5)
        .set_log_level("info")
        .start().unwrap();

    let data_folder = std::env::current_dir().unwrap();
    let default_config = data_folder.join("config.toml").to_string_lossy().to_string();
    let matches = clap::Command::new("vpn-server")
        .version("0.1.0")
        .about("vpn server")
        .arg(clap::Arg::new("config")
            .short('c')
            .long("config")
            .value_name("FILE")
            .help("Sets a custom config file")
            .required(false))
        .subcommand(clap::Command::new("join")
            .arg(clap::Arg::new("server")
                .long("server")
                .short('s')
                .help("The vpn server ip")
                .required(true))
            .arg(clap::Arg::new("server_id")
                .long("server_id")
                .help("The vpn server id")
                .required(true))
            .arg(clap::Arg::new("network_id")
                .long("network_id")
                .value_parser(clap::value_parser!(u64))
                .help("The network id you want to join")
                .required(true))
            .arg(clap::Arg::new("name")
                .long("name")
                .short('n')
                .help("The name of the node seen on the server")
                .required(false)))
        .subcommand(clap::Command::new("state"))
        .get_matches();

    match matches.subcommand() {
        Some(("join", matches)) => {
            let server = matches.get_one::<String>("server").unwrap();
            let server_id = matches.get_one::<String>("server_id").unwrap();
            let id = matches.get_one::<u64>("network_id").unwrap();
            let name = matches.get_one::<String>("name");
            Cli::join(server.clone(), server_id.clone(), *id, name.map(|v| v.clone())).await;
            return;
        },
        Some(("state", _)) => {

        }
        _ => {}
    }

    let eps = vec![Endpoint::from((Protocol::Quic, SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 3422))))];
    let p2p_config = P2pConfig::new(Arc::new(X509IdentityFactory), Arc::new(X509IdentityCertFactory), eps);
    init_p2p(p2p_config).await.unwrap();

    let vpn_config_path = dirs::data_dir().unwrap().join("vpn");
    if !vpn_config_path.exists() {
        let _ = create_dir_all(vpn_config_path.as_path());
    }

    init_p2p_vpn_client_manager(vpn_config_path.clone(), 34245, 3424, "1.0.0".to_string()).unwrap();

    let setting = Arc::new(Setting::load(vpn_config_path.join("setting.toml").as_path()).await.unwrap());
    if let Some(records) = setting.get::<Vec<JoinRecord>>("joined_networks") {
        for record in records.iter() {
            let vpn_client = vpn_client_manager().get_client(format!("{}_{}", record.server_id, record.server_ip).as_str()).await.unwrap();
            vpn_client.run();
        }
    }

    // let local_identity = x509::generate_x509_identity(None).unwrap();
    // let conn_timeout = Duration::from_secs(30);
    // let stack_config = P2pStackConfig::new(Arc::new(local_identity))
    //     .set_conn_timeout(conn_timeout)
    //     .set_support_proxy(true);
    // let stack = create_p2p_stack(stack_config).await.unwrap();
    // stack.wait_online(None).await.unwrap();

    let http_config = HttpServerConfig::new("127.0.0.1", 45364);
    let mut http_server = TideHttpServer::new(http_config);
    Api::register_api(&mut http_server, setting);
    http_server.run().await.unwrap();
    std::future::pending::<()>().await;
}
