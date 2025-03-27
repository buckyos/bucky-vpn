mod api;
mod cli;
mod p2p_vpn;
mod setting;

#[cfg(target_os = "windows")]
mod windows_main;

use std::fs::create_dir_all;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::Arc;
use config::builder::DefaultState;
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::stack::{init_p2p, P2pConfig};
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use sfo_http::http_server::HttpServerConfig;
use sfo_http::tide_server::TideHttpServer;
use crate::api::Api;
use crate::cli::Cli;
use crate::p2p_vpn::{init_p2p_vpn_client_manager, vpn_client_manager, JoinRecord};
use crate::setting::Setting;

async fn async_main() {
    let mut config = config::ConfigBuilder::<DefaultState>::default();

    config = config.add_source(config::Environment::with_prefix("VPN").separator("_"));
    let config = config.build().unwrap();

    let vpn_config_path = match config.get_string("data.dir") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            dirs::data_dir().unwrap().join("bucky-vpn")
        }
    };
    let sn_port = config.get_int("port").unwrap_or(3624) as u16;
    let p2p_port = config.get_int("p2p.port").unwrap_or(3622) as u16;

    let matches = clap::Command::new("bucky-vpn")
        .about("bucky-vpn")
        .arg_required_else_help(true)
        .subcommand(clap::Command::new("join")
            .about("join a vpn network")
            .arg(clap::Arg::new("server")
                .long("server")
                .short('s')
                .help("The vpn server ip")
                .required(true))
            .arg(clap::Arg::new("port")
                .long("port")
                .short('p')
                .help("The vpn server port")
                .required(false))
            .arg(clap::Arg::new("server_id")
                .long("server_id")
                .help("The vpn server identity ID")
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
        // .subcommand(clap::Command::new("state"))
        .subcommand(clap::Command::new("daemon").about("Run as vpn service"))
        .get_matches();

    match matches.subcommand() {
        Some(("join", matches)) => {
            let server = matches.get_one::<String>("server").unwrap();
            let server_id = matches.get_one::<String>("server_id").unwrap();
            let id = matches.get_one::<u64>("network_id").unwrap();
            let name = matches.get_one::<String>("name");
            let server_port = matches.get_one::<u16>("port").unwrap_or(&sn_port);
            match Cli::join(server.clone(), *server_port, server_id.clone(), *id, name.map(|v| v.clone())).await {
                Ok(_) => {
                    println!("Join success");
                }
                Err(_e) => {
                    println!("Join failed");
                }
            }
            return;
        },
        Some(("state", _)) => {

        },
        Some(("daemon", _)) => {
            let log = config.get_bool("log").unwrap_or(true);
            if log {
                sfo_log::Logger::new("bucky-vpn")
                    .set_log_to_file(true)
                    .set_log_file_count(5)
                    .set_log_path(vpn_config_path.join("logs").to_string_lossy().to_string().as_str())
                    .set_log_level("info")
                    .start().unwrap();
            }

            let eps = vec![Endpoint::from((Protocol::Quic, SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, p2p_port))))];
            let p2p_config = P2pConfig::new(Arc::new(X509IdentityFactory), Arc::new(X509IdentityCertFactory), eps);
            init_p2p(p2p_config).await.unwrap();

            if !vpn_config_path.exists() {
                let _ = create_dir_all(vpn_config_path.as_path());
            }

            init_p2p_vpn_client_manager(vpn_config_path.clone(), 34245, "1.0.0".to_string()).unwrap();

            let setting = Arc::new(Setting::load(vpn_config_path.join("setting.toml").as_path()).await.unwrap());
            if let Some(records) = setting.get::<Vec<JoinRecord>>("joined_networks") {
                for record in records.iter() {
                    let vpn_client = vpn_client_manager().get_client(format!("{}_{}:{}", record.server_id, record.server_ip, record.server_port).as_str()).await.unwrap();
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

            let http_config = HttpServerConfig::new("127.0.0.1", 4536);
            let mut http_server = TideHttpServer::new(http_config);
            Api::register_api(&mut http_server, setting);
            http_server.run().await.unwrap();
            std::future::pending::<()>().await;
        }
        _ => {
        }
    }

}

fn main() {
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    {
        windows_main::windows_main().unwrap();
    }

    #[cfg(any(not(target_os = "windows"), debug_assertions))]
    {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async_main())
    }
}
