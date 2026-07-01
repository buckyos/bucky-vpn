use crate::api::Api;
use crate::pn_connection_validator::VpnServerPnConnectionValidator;
use crate::pn_traffic_service::{
    NoopPnTrafficSnapshotProvider, PnTrafficNodeSet, PnTrafficService,
    TrackedPnTrafficSnapshotProvider,
};
use crate::server_config::{
    ConfigPnServerSelector, build_server_config, endpoint_to_pn_server, get_pn_server_config,
    get_sn_server_config, is_standalone_proxy_node, resolve_service_endpoints,
    select_default_config_file, should_start_pn_server,
};
use crate::sqlite_store_factory::{P2pSnCmdServer, SqliteStoreFactory};
use crate::user_store::{SqliteUserStore, User};
use crate::vpn_control_client::{
    VpnCmdIncomingTunnelValidator, VpnCmdPnConnectionValidator, VpnCmdPnTrafficReporter,
    create_vpn_control_client, reject_all_incoming_tunnel_validator,
};
use base58::ToBase58;
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::p2p_identity::{P2pIdentity, P2pIdentityFactory, P2pIdentityRef};
use p2p_frame::pn::PnServer;
use p2p_frame::sn::service::{SnServiceConfig, create_sn_service};
use p2p_frame::stack::{P2pConfig, create_p2p_env};
use p2p_frame::ttp::{TtpServer, TtpServerRef};
use p2p_frame::x509;
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use sfo_account::{AccountServer, AccountStore, DefaultAccountManager, hash_data};
use sfo_http::http_server::HttpServerConfig;
use sfo_http::openapi::OpenApiServer;
use sfo_http::openapi::utoipa;
use sfo_http::openapi::utoipa::OpenApi;
use sfo_http::tide_server::TideHttpServer;
use sfo_sql::sqlite::{SqlPool, SqliteJournalMode};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use vpn_frame::server::{NodeId, VpnServer, VpnStoreFactory};

mod api;
mod pn_connection_validator;
mod pn_traffic_service;
mod server_config;
mod sqlite_store_factory;
mod user_store;
mod vpn_control_client;

#[derive(utoipa::OpenApi)]
#[openapi(paths(), components())]
struct ApiDoc;

async fn start_proxy_ttp_server(
    local_identity: P2pIdentityRef,
    endpoints: Vec<Endpoint>,
    incoming_tunnel_validator: p2p_frame::networks::IncomingTunnelValidatorRef,
) -> p2p_frame::error::P2pResult<TtpServerRef> {
    let p2p_env = create_p2p_env(
        P2pConfig::new(
            Arc::new(X509IdentityFactory),
            Arc::new(X509IdentityCertFactory),
            endpoints,
        )
        .set_incoming_tunnel_validator(incoming_tunnel_validator),
    )
    .await?;
    p2p_env
        .net_manager()
        .add_listen_device(local_identity.clone())
        .await?;
    let ttp_server = TtpServer::new(local_identity, p2p_env.net_manager().clone())?;
    p2p_env
        .net_manager()
        .listen(p2p_env.endpoints(), p2p_env.port_mapping().clone())
        .await?;
    Ok(ttp_server)
}

#[tokio::main]
async fn main() {
    let data_folder = std::env::current_dir().unwrap();
    let default_config = select_default_config_file(data_folder.as_path())
        .to_string_lossy()
        .to_string();
    let matches = clap::Command::new("vpn-server")
        .version("0.1.0")
        .about("vpn server")
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .required(false),
        )
        .get_matches();

    let explicit_config_file = matches.get_one::<String>("config").map(String::as_str);
    let config_file = explicit_config_file.unwrap_or(default_config.as_str());
    let config = build_server_config(explicit_config_file, data_folder.as_path()).unwrap();
    let sn_server_config = get_sn_server_config(&config);
    let pn_config = get_pn_server_config(&config).unwrap();

    let ip = config.get_string("ip").unwrap();
    let port = config.get_int("port").unwrap() as u16;
    let http_ip = config.get_string("http.ip").unwrap();
    let http_port = config.get_int("http.port").unwrap() as u16;
    let admin_name = config.get_string("admin.name").unwrap();
    let admin_password = config.get_string("admin.password").unwrap();
    let jwt_key = config.get_string("jwt.key").unwrap().to_string();
    let data_dir = match config.get_string("data.dir") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => dirs::data_dir().unwrap().join("bucky-vpn-server"),
    };

    let log = config.get_bool("log").unwrap_or(true);
    let log_level = config
        .get_string("log.level")
        .unwrap_or(String::from("info"));
    if log {
        sfo_log::Logger::new("vpn-server")
            .set_log_to_file(true)
            .set_log_file_count(5)
            .set_log_path(data_dir.join("logs").to_string_lossy().to_string().as_str())
            .set_log_level(log_level.as_str())
            .add_filter("quinn")
            .start()
            .unwrap();
    }

    if !data_dir.exists() {
        tokio::fs::create_dir_all(data_dir.as_path()).await.unwrap();
    }
    let sn_endpoint = Endpoint::from((
        Protocol::Quic,
        SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from_str(ip.as_str()).unwrap(),
            port,
        )),
    ));
    let eps = resolve_service_endpoints(sn_endpoint.clone(), &sn_server_config, &pn_config);
    let start_pn_server = should_start_pn_server(&sn_server_config, &pn_config);
    let standalone_proxy_node = is_standalone_proxy_node(&sn_server_config, &pn_config);

    let identity_file = data_dir.join("identity");
    let identity = if identity_file.exists() {
        let data = tokio::fs::read(identity_file.as_path()).await.unwrap();
        X509IdentityFactory.create(&data).unwrap()
    } else {
        let local_identity = x509::generate_rsa_x509_identity(None).unwrap();
        let data = local_identity.get_encoded_identity().unwrap();
        tokio::fs::write(identity_file.as_path(), data)
            .await
            .unwrap();
        Arc::new(local_identity)
    };
    let local_identity = identity.update_endpoints(eps.clone());
    let control_identity = local_identity.clone();
    let local_id = local_identity.get_id();
    let local_id_string = local_id.to_string();
    let local_pn_server = endpoint_to_pn_server(&local_id_string, &sn_endpoint);
    let pn_servers = if start_pn_server {
        eps.iter()
            .map(|endpoint| endpoint_to_pn_server(&local_id_string, endpoint))
            .collect()
    } else {
        Vec::new()
    };
    let remote_control_client = if standalone_proxy_node {
        match pn_config.control_server.as_ref() {
            Some(control_server) => match create_vpn_control_client(
                control_identity.clone(),
                control_server,
                std::time::Duration::from_secs(5),
            )
            .await
            {
                Ok(client) => Some(client),
                Err(err) => {
                    log::error!(
                        "create vpn control client failed: code={:?} msg={}",
                        err.code(),
                        err.msg()
                    );
                    None
                }
            },
            None => {
                log::warn!(
                    "standalone proxy node requires pn.control_server for remote tunnel validation"
                );
                None
            }
        }
    } else {
        None
    };
    let sn_service_config = SnServiceConfig::new(
        local_identity,
        Arc::new(X509IdentityFactory),
        Arc::new(X509IdentityCertFactory),
    );
    let sn_service = create_sn_service(sn_service_config).await;
    let pn_ttp_server = if sn_server_config.enabled {
        sn_service.start().await.unwrap();
        Some(sn_service.ttp_server())
    } else if standalone_proxy_node {
        let incoming_tunnel_validator: p2p_frame::networks::IncomingTunnelValidatorRef =
            if let Some(client) = remote_control_client.as_ref() {
                VpnCmdIncomingTunnelValidator::new(client.clone())
            } else {
                reject_all_incoming_tunnel_validator()
            };
        let ttp_server = start_proxy_ttp_server(
            control_identity.clone(),
            eps.clone(),
            incoming_tunnel_validator,
        )
        .await
        .unwrap();
        log::info!(
            "default sn server disabled by config file {}, standalone proxy ttp listener started",
            config_file
        );
        Some(ttp_server)
    } else {
        log::info!("default sn server disabled by config file {}", config_file);
        None
    };

    let control_runtime = if sn_server_config.enabled {
        let db_path = data_dir.join("vpn.db").to_string_lossy().to_string();
        let pool = SqlPool::open(db_path.as_str(), 5, Some(SqliteJournalMode::Wal))
            .await
            .unwrap();

        let user_store = SqliteUserStore::new(pool.clone());
        user_store.init_user_store().await.unwrap();

        let store_factory = Arc::new(SqliteStoreFactory::from_pool(pool));
        {
            let mut store = store_factory.get_vpn_store().await.unwrap();
            store.init_db().await.unwrap();
        }

        let pn_server_selector = Arc::new(ConfigPnServerSelector::new_with_store(
            pn_servers,
            store_factory.clone(),
        ));
        let vpn_server = VpnServer::new_with_pn_server_selector(
            Arc::new(P2pSnCmdServer::new(sn_service.clone())),
            store_factory.clone(),
            pn_server_selector.clone(),
        );
        let network_manager = vpn_server.network_manager().clone();

        user_store.update_password(
            &admin_name,
            hash_data(vec![admin_name.as_bytes(), admin_password.as_bytes()].as_slice())
                .to_base58()
                .as_str(),
        );
        if user_store.get_account(&admin_name).await.unwrap().is_none() {
            let network_id = network_manager.new_network_group().await.unwrap();
            let user = User {
                id: admin_name.clone(),
                password: hash_data(
                    vec![admin_name.as_bytes(), admin_password.as_bytes()].as_slice(),
                )
                .to_base58(),
                network_id,
                server_id: local_id.to_string(),
            };
            user_store.add_account(&user).await.unwrap();
        }

        let user_manager = DefaultAccountManager::new(user_store, jwt_key.into_bytes());

        vpn_server.start();
        Some((store_factory, user_manager, vpn_server, pn_server_selector))
    } else {
        None
    };

    let traffic_service = if start_pn_server {
        let traffic_node_set = PnTrafficNodeSet::new();
        let pn_validator: p2p_frame::pn::PnConnectionValidatorRef =
            if let Some(client) = remote_control_client.as_ref() {
                VpnCmdPnConnectionValidator::new_with_traffic_node_tracker(
                    client.clone(),
                    traffic_node_set.clone(),
                )
            } else if let Some((_, _, vpn_server, _)) = control_runtime.as_ref() {
                VpnServerPnConnectionValidator::new(
                    vpn_server.clone(),
                    NodeId::from(local_id.as_slice()),
                )
            } else {
                p2p_frame::pn::reject_all_pn_connection_validator()
            };
        let pn_server =
            PnServer::new_with_connection_validator(pn_ttp_server.clone().unwrap(), pn_validator);
        pn_server.start().await.unwrap();
        let traffic_snapshot_provider =
            TrackedPnTrafficSnapshotProvider::new(pn_server.clone(), traffic_node_set);
        let traffic_service = if let Some((store_factory, _, _, _)) = control_runtime.as_ref() {
            PnTrafficService::new(traffic_snapshot_provider, store_factory.clone())
        } else {
            PnTrafficService::new_without_store(traffic_snapshot_provider)
        };
        if let Some(client) = remote_control_client {
            traffic_service.set_remote_reporter(VpnCmdPnTrafficReporter::new(
                client,
                local_pn_server.clone(),
            ));
            traffic_service.start_remote_heartbeat(
                NodeId::from(local_id.as_slice()),
                std::time::Duration::from_secs(pn_config.report_interval_secs),
            );
            log::info!(
                "proxy node heartbeat and traffic report enabled by vpn command interval_secs={}",
                pn_config.report_interval_secs
            );
        }
        traffic_service.start_background_flush(std::time::Duration::from_secs(
            pn_config.report_interval_secs,
        ));
        traffic_service
    } else {
        log::info!("default pn server disabled by config file {}", config_file);
        if let Some((store_factory, _, _, _)) = control_runtime.as_ref() {
            PnTrafficService::new(
                Arc::new(NoopPnTrafficSnapshotProvider),
                store_factory.clone(),
            )
        } else {
            PnTrafficService::new_without_store(Arc::new(NoopPnTrafficSnapshotProvider))
        }
    };

    if let Some((_, user_manager, vpn_server, pn_server_selector)) = control_runtime {
        let http_config = HttpServerConfig::new(http_ip, http_port)
            .allow_any_header()
            .allow_any_origin()
            .allow_any_methods()
            .expose_any_header();
        let mut http_server = TideHttpServer::new(http_config);
        http_server.set_api_doc(ApiDoc::openapi());
        http_server.enable_api_doc(true);

        AccountServer::register_server(&mut http_server, user_manager.clone());
        Api::register_api(
            &mut http_server,
            user_manager.clone(),
            vpn_server.clone(),
            traffic_service.clone(),
            pn_server_selector.clone(),
        );

        http_server.run().await.unwrap();
    }

    std::future::pending::<()>().await;
}
