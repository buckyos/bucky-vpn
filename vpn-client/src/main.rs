mod api;
mod cli;
mod p2p_vpn;
mod setting;

#[cfg(target_os = "windows")]
mod windows_main;

use crate::api::Api;
use crate::cli::Cli;
use crate::p2p_vpn::{JoinRecord, init_p2p_vpn_client_manager, vpn_client_manager};
use crate::setting::Setting;
use config::builder::DefaultState;
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::stack::{P2pConfig, create_p2p_env};
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use sfo_http::http_server::HttpServerConfig;
use sfo_http::tide_server::TideHttpServer;
use std::fs::create_dir_all;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const SN_PORT: u16 = 3624;

async fn run_daemon() {
    let mut config = config::ConfigBuilder::<DefaultState>::default();

    config = config.add_source(config::Environment::with_prefix("VPN").separator("_"));
    let config = config.build().unwrap();

    let vpn_config_path = match config.get_string("data.dir") {
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
    };

    let p2p_port = config.get_int("p2p.port").unwrap_or(3622) as u16;

    if !vpn_config_path.exists() {
        let _ = create_dir_all(vpn_config_path.as_path());
    }

    let log = config.get_bool("log").unwrap_or(true);
    let log_level = config
        .get_string("log.level")
        .unwrap_or(String::from("info"));
    if log {
        sfo_log::Logger::new("bucky-vpn")
            .set_log_to_file(true)
            .set_log_file_count(5)
            .set_log_path(
                vpn_config_path
                    .join("logs")
                    .to_string_lossy()
                    .to_string()
                    .as_str(),
            )
            .set_log_level(log_level.as_str())
            .start()
            .unwrap();
    }

    let eps = vec![Endpoint::from((
        Protocol::Quic,
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, p2p_port)),
    ))];
    let p2p_config = P2pConfig::new(
        Arc::new(X509IdentityFactory),
        Arc::new(X509IdentityCertFactory),
        eps,
    )
    .set_quic_connect_timeout(Duration::from_secs(8))
    .set_quic_idle_time(Duration::from_secs(30));
    let p2p_env = create_p2p_env(p2p_config).await.unwrap();

    init_p2p_vpn_client_manager(p2p_env, vpn_config_path.clone(), 34245, "1.0.0".to_string())
        .unwrap();

    let setting = Arc::new(
        Setting::load(vpn_config_path.join("setting.toml").as_path())
            .await
            .unwrap(),
    );
    if let Some(records) = setting.get::<Vec<JoinRecord>>("joined_networks") {
        for record in records {
            tokio::spawn(async move {
                let mut interval = 5;
                loop {
                    let vpn_client = match vpn_client_manager()
                        .get_client(
                            format!(
                                "{}_{}:{}",
                                record.server_id, record.server_ip, record.server_port
                            )
                            .as_str(),
                        )
                        .await
                    {
                        Ok(v) => v,
                        Err(_) => {
                            log::error!("get client failed");
                            interval *= 2;
                            if interval > 3600 {
                                interval = 3600;
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                            continue;
                        }
                    };
                    vpn_client.run();
                    break;
                }
            });
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
fn main() {
    let matches = clap::Command::new("bucky-vpn")
        .about("bucky-vpn")
        .arg_required_else_help(true)
        .subcommand(
            clap::Command::new("join")
                .about("join a vpn network")
                .arg(
                    clap::Arg::new("server")
                        .long("server")
                        .short('s')
                        .help("The vpn server ip or domain")
                        .required(true),
                )
                .arg(
                    clap::Arg::new("port")
                        .long("port")
                        .short('p')
                        .help("The vpn server port")
                        .required(false),
                )
                .arg(
                    clap::Arg::new("server_id")
                        .long("server_id")
                        .help("The vpn server identity ID")
                        .required(true),
                )
                .arg(
                    clap::Arg::new("network_id")
                        .long("network_id")
                        .value_parser(clap::value_parser!(u64))
                        .help("The network id you want to join")
                        .required(true),
                )
                .arg(
                    clap::Arg::new("name")
                        .long("name")
                        .short('n')
                        .help("The name of the node seen on the server")
                        .required(false),
                ),
        )
        // .subcommand(clap::Command::new("state"))
        .subcommand(clap::Command::new("daemon").about("Run as vpn service"))
        .get_matches();

    match matches.subcommand() {
        Some(("join", matches)) => {
            let server = matches.get_one::<String>("server").unwrap().clone();
            let server_id = matches.get_one::<String>("server_id").unwrap().clone();
            let id = matches.get_one::<u64>("network_id").unwrap().clone();
            let name = matches.get_one::<String>("name").map(|v| v.clone());
            let server_port = matches.get_one::<u16>("port").unwrap_or(&SN_PORT).clone();
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    match Cli::join(server, server_port, server_id, id, name).await {
                        Ok(_) => {
                            println!("Join success");
                        }
                        Err(_e) => {
                            println!("Join failed.{}", _e.msg());
                        }
                    }
                });
            return;
        }
        Some(("state", _)) => {}
        Some(("daemon", _)) => {
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
                    .block_on(run_daemon());
            }
        }
        _ => {}
    }
}
