use crate::api::Api;
use crate::pn_connection_validator::VpnServerPnConnectionValidator;
use crate::pn_server_info::with_pn_server_name;
use crate::pn_server_manager::PnServerManager;
use crate::pn_traffic_service::PnTrafficService;
use crate::server_config::{
    build_server_config, endpoints_to_pn_server, get_node_traffic_idempotency_retention_secs,
    get_pn_server_config, get_pn_traffic_upload_config, get_server_name_config,
    get_sn_admin_config, get_sn_http_config, get_sn_jwt_config, get_sn_server_config,
    is_standalone_proxy_node, resolve_service_endpoints, select_default_config_file,
    should_start_pn_server, validate_server_mode,
};
use crate::sqlx_store::open_sqlite_pool;
use crate::sqlite_store_factory::{P2pSnCmdServer, SqliteStoreFactory};
use crate::user_store::{SqliteUserStore, User};
use crate::pn_control_client::{
    DeferredVpnCmdIncomingTunnelValidator, LocalPnTrafficReporter, VpnCmdPnConnectionValidator,
    VpnCmdPnTrafficReporter, create_vpn_control_client,
};
use crate::pn_control_server::{
    create_proxy_control_cmd_service, create_proxy_control_tunnel_observer,
    register_proxy_control_cmd_listener,
};
use base58::ToBase58;
use bucky_raw_codec::{RawConvertTo, RawDecode, RawEncode, RawFrom};
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::p2p_identity::{P2pIdentity, P2pIdentityFactory, P2pIdentityRef, P2pSn};
use p2p_frame::pn::PnServer;
use p2p_frame::sn::service::{SnServiceConfig, create_sn_service};
use p2p_frame::stack::{P2pConfig, create_p2p_env};
use p2p_frame::ttp::{TtpClient, TtpServer, TtpServerRef};
use p2p_frame::x509;
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use rcgen::KeyPair;
use sfo_account::{AccountServer, AccountStore, DefaultAccountManager, hash_data};
use sfo_http::http_server::HttpServerConfig;
use sfo_http::openapi::OpenApiServer;
use sfo_http::openapi::utoipa;
use sfo_http::openapi::utoipa::OpenApi;
use sfo_http::tide_server::TideHttpServer;
use sfo_reuseport::{ServerRuntime, ServerRuntimeConfig};
use sqlx::sqlite::SqliteJournalMode;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use vpn_frame::errors::VpnResult;
use vpn_frame::server::{NodeId, VpnServer, VpnStoreFactory};

mod api;
mod pn_connection_validator;
mod pn_control_client;
mod pn_control_server;
mod pn_server_info;
mod pn_server_manager;
mod pn_traffic_service;
mod server_config;
mod sqlx_store;
mod sqlite_store_factory;
mod user_store;

#[derive(utoipa::OpenApi)]
#[openapi(paths(), components())]
struct ApiDoc;

#[derive(Debug, Clone, RawEncode, RawDecode)]
struct EncodedX509IdentityCertData {
    raw_cert: Vec<u8>,
    sn_list: Vec<P2pSn>,
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, RawEncode, RawDecode)]
struct EncodedX509IdentityData {
    key: Vec<u8>,
    cert: EncodedX509IdentityCertData,
}

async fn load_or_create_identity(identity_file: &PathBuf, name: Option<String>) -> P2pIdentityRef {
    if identity_file.exists() {
        let data = tokio::fs::read(identity_file.as_path()).await.unwrap();
        let identity = X509IdentityFactory.create(&data).unwrap();
        if let Some(name) = name {
            if identity.get_name() != name {
                let data = resign_encoded_identity_with_name(data.as_slice(), &name).unwrap();
                tokio::fs::write(identity_file.as_path(), data.as_slice())
                    .await
                    .unwrap();
                return X509IdentityFactory.create(&data).unwrap();
            }
        }
        identity
    } else {
        let local_identity = x509::generate_rsa_x509_identity(name).unwrap();
        let data = local_identity.get_encoded_identity().unwrap();
        tokio::fs::write(identity_file.as_path(), data)
            .await
            .unwrap();
        Arc::new(local_identity)
    }
}

fn resign_encoded_identity_with_name(data: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut identity_data = EncodedX509IdentityData::clone_from_slice(data)
        .map_err(|err| format!("decode identity failed: {err:?}"))?;
    let key_pair = KeyPair::try_from(identity_data.key.clone())
        .map_err(|err| format!("parse identity private key failed: {err:?}"))?;
    let renamed_identity =
        x509::generate_x509_identity_with_key_pair(Some(name.to_owned()), key_pair)
            .map_err(|err| format!("regenerate identity certificate failed: {err:?}"))?;
    let renamed_identity_data = renamed_identity
        .get_encoded_identity()
        .map_err(|err| format!("encode regenerated identity failed: {err:?}"))?;
    let renamed_identity_data =
        EncodedX509IdentityData::clone_from_slice(renamed_identity_data.as_slice())
            .map_err(|err| format!("decode regenerated identity failed: {err:?}"))?;
    identity_data.cert.raw_cert = renamed_identity_data.cert.raw_cert;
    identity_data
        .to_vec()
        .map_err(|err| format!("encode identity failed: {err:?}"))
}

struct ProxyTtpRuntime {
    server: TtpServerRef,
}

fn new_p2p_config(endpoints: Vec<Endpoint>, server_runtime: ServerRuntime) -> P2pConfig {
    P2pConfig::new(
        Arc::new(X509IdentityFactory),
        Arc::new(X509IdentityCertFactory),
        endpoints,
        server_runtime,
    )
}

fn new_sn_service_config(
    local_identity: P2pIdentityRef,
    server_runtime: ServerRuntime,
) -> SnServiceConfig {
    SnServiceConfig::new(
        local_identity,
        Arc::new(X509IdentityFactory),
        Arc::new(X509IdentityCertFactory),
        server_runtime,
    )
}

async fn start_standalone_proxy_ttp_runtime(
    local_identity: P2pIdentityRef,
    endpoints: Vec<Endpoint>,
    incoming_tunnel_validator: Arc<DeferredVpnCmdIncomingTunnelValidator>,
    control_server: &crate::server_config::PnControlServerConfig,
    server_runtime: ServerRuntime,
) -> Result<(ProxyTtpRuntime, crate::pn_control_client::VpnControlClientRef), String> {
    let p2p_env = create_p2p_env(
        new_p2p_config(endpoints, server_runtime)
            .set_incoming_tunnel_validator(incoming_tunnel_validator.clone()),
    )
    .await
    .map_err(|err| err.to_string())?;
    p2p_env
        .net_manager()
        .add_listen_device(local_identity.clone())
        .await
        .map_err(|err| err.to_string())?;
    let net_manager = p2p_env.net_manager().clone();
    let ttp_server = TtpServer::new(local_identity.clone(), net_manager.clone())
        .map_err(|err| err.to_string())?;
    let ttp_client = TtpClient::new(local_identity, net_manager);
    let control_client = create_vpn_control_client(
        ttp_client.clone(),
        control_server,
        std::time::Duration::from_secs(5),
    )
    .await
    .map_err(|err| err.to_string())?;
    incoming_tunnel_validator.set_client(control_client.clone());
    p2p_env
        .net_manager()
        .listen(p2p_env.endpoints(), p2p_env.port_mapping().clone())
        .await
        .map_err(|err| err.to_string())?;
    Ok((
        ProxyTtpRuntime {
            server: ttp_server,
        },
        control_client,
    ))
}

pub(crate) async fn drain_startup_expired_node_traffic_reports(
    factory: &SqliteStoreFactory,
) -> VpnResult<u64> {
    const CLEANUP_BATCH_SIZE: usize = 1024;

    let Some(cutoff_ms) = factory.expiration_cutoff_ms()? else {
        return Ok(0);
    };
    let mut total_deleted = 0;
    loop {
        let deleted = factory
            .cleanup_expired_node_traffic_reports(cutoff_ms, CLEANUP_BATCH_SIZE)
            .await?;
        total_deleted += deleted;
        if deleted < CLEANUP_BATCH_SIZE as u64 {
            return Ok(total_deleted);
        }
        tokio::task::yield_now().await;
    }
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
    let server_name = get_server_name_config(&config);
    let sn_server_config = get_sn_server_config(&config);
    let pn_config = get_pn_server_config(&config).unwrap();
    validate_server_mode(&sn_server_config, &pn_config).unwrap();
    let node_traffic_idempotency_retention_secs =
        get_node_traffic_idempotency_retention_secs(&config).unwrap();
    let node_traffic_idempotency_retention =
        std::time::Duration::from_secs(node_traffic_idempotency_retention_secs);
    let node_traffic_speed_ttl_secs = pn_config
        .report_interval_secs
        .checked_mul(3)
        .unwrap_or_else(|| {
            panic!(
                "pn.report_interval_secs={} cannot produce a safe node traffic speed ttl",
                pn_config.report_interval_secs
            )
        });
    let node_traffic_speed_ttl =
        std::time::Duration::from_secs(node_traffic_speed_ttl_secs);
    let pn_traffic_upload_config = get_pn_traffic_upload_config(&config).unwrap();
    let sn_http_config = if sn_server_config.enabled {
        Some(get_sn_http_config(&config).unwrap())
    } else {
        None
    };
    let sn_admin_config = if sn_server_config.enabled {
        Some(get_sn_admin_config(&config).unwrap())
    } else {
        None
    };
    let sn_jwt_config = if sn_server_config.enabled {
        Some(get_sn_jwt_config(&config).unwrap())
    } else {
        None
    };

    let ip = config.get_string("ip").unwrap();
    let port = config.get_int("port").unwrap() as u16;
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
    let identity = load_or_create_identity(&identity_file, server_name.clone()).await;
    let local_identity = identity.update_endpoints(eps.clone());
    let control_identity = local_identity.clone();
    let local_id = local_identity.get_id();
    let local_id_string = local_id.to_string();
    let pn_route_hint = pn_config
        .control_server
        .as_ref()
        .map(|control_server| &control_server.endpoint);
    let local_pn_server = endpoints_to_pn_server(
        &local_id_string,
        &sn_endpoint,
        &eps,
        pn_route_hint,
        pn_config.advertised_ip,
        &pn_config.port_mapping,
        pn_config.report_local_address,
    );
    let local_pn_server = with_pn_server_name(local_pn_server, server_name.clone()).unwrap();
    let pn_servers = if start_pn_server {
        vec![local_pn_server.clone()]
    } else {
        Vec::new()
    };
    let mut remote_control_client = None;
    let server_runtime = ServerRuntime::start(ServerRuntimeConfig::default()).unwrap();
    let sn_service_config = new_sn_service_config(local_identity, server_runtime.clone());
    let sn_service = create_sn_service(sn_service_config).await.unwrap();
    let pn_ttp_server = if sn_server_config.enabled {
        sn_service.start().await.unwrap();
        Some(sn_service.ttp_server())
    } else if standalone_proxy_node {
        let incoming_tunnel_validator = DeferredVpnCmdIncomingTunnelValidator::new();
        let control_server = pn_config
            .control_server
            .as_ref()
            .expect("standalone proxy mode was validated before runtime construction");
        let (ttp_runtime, control_client) = start_standalone_proxy_ttp_runtime(
            control_identity.clone(),
            eps.clone(),
            incoming_tunnel_validator.clone(),
            control_server,
            server_runtime.clone(),
        )
        .await
        .unwrap();
        remote_control_client = Some(control_client);
        log::info!(
            "default sn server disabled by config file {}, standalone proxy ttp listener started",
            config_file
        );
        Some(ttp_runtime.server)
    } else {
        log::info!("default sn server disabled by config file {}", config_file);
        None
    };

    let control_runtime = if sn_server_config.enabled {
        let sn_admin_config = sn_admin_config
            .as_ref()
            .expect("sn admin config is parsed when sn is enabled");
        let sn_jwt_config = sn_jwt_config
            .as_ref()
            .expect("sn jwt config is parsed when sn is enabled");
        let db_path = data_dir.join("vpn.db").to_string_lossy().to_string();
        let pool = open_sqlite_pool(db_path.as_str(), 5, Some(SqliteJournalMode::Wal))
            .await
            .unwrap();

        let user_store = SqliteUserStore::new(pool.clone());
        user_store.init_user_store().await.unwrap();

        let store_factory = Arc::new(
            SqliteStoreFactory::from_pool_with_node_traffic_settings(
                pool,
                node_traffic_idempotency_retention,
                node_traffic_speed_ttl,
            ),
        );
        {
            let mut store = store_factory.get_vpn_store().await.unwrap();
            store.init_db().await.unwrap();
        }
        drain_startup_expired_node_traffic_reports(store_factory.as_ref())
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "startup cleanup of expired pn node traffic reports failed: code={:?} msg={}",
                    err.code(),
                    err.msg()
                )
            });

        let pn_server_selector = Arc::new(PnServerManager::new_with_store_and_remote_ttl(
            pn_servers,
            store_factory.clone(),
            std::time::Duration::from_secs(pn_config.heartbeat_timeout_secs),
        ));
        pn_server_selector.start_remote_liveness_monitor();
        let cmd_server = Arc::new(P2pSnCmdServer::new(sn_service.clone()));
        let proxy_control_cmd_service = create_proxy_control_cmd_service();
        let proxy_control_tunnel_observer =
            create_proxy_control_tunnel_observer(proxy_control_cmd_service.clone());
        let vpn_server = VpnServer::new_with_pn_control_cmd_server_and_observer(
            cmd_server,
            proxy_control_cmd_service.clone(),
            store_factory.clone(),
            pn_server_selector.clone(),
            proxy_control_tunnel_observer,
        );
        let network_manager = vpn_server.network_manager().clone();

        user_store.update_password(
            &sn_admin_config.name,
            hash_data(
                vec![
                    sn_admin_config.name.as_bytes(),
                    sn_admin_config.password.as_bytes(),
                ]
                .as_slice(),
            )
            .to_base58()
            .as_str(),
        );
        if user_store
            .get_account(&sn_admin_config.name)
            .await
            .unwrap()
            .is_none()
        {
            let network_id = network_manager.new_network_group().await.unwrap();
            let user = User {
                id: sn_admin_config.name.clone(),
                password: hash_data(
                    vec![
                        sn_admin_config.name.as_bytes(),
                        sn_admin_config.password.as_bytes(),
                    ]
                    .as_slice(),
                )
                .to_base58(),
                network_id,
                server_id: local_id.to_string(),
            };
            user_store.add_account(&user).await.unwrap();
        }

        let user_manager =
            DefaultAccountManager::new(user_store, sn_jwt_config.key.clone().into_bytes());

        vpn_server.start();
        if let Err(err) =
            register_proxy_control_cmd_listener(sn_service.ttp_server(), proxy_control_cmd_service)
                .await
        {
            log::error!(
                "start proxy control listener failed: code={:?} msg={}",
                err.code(),
                err.msg()
            );
            panic!("start proxy control listener failed");
        }
        Some((store_factory, user_manager, vpn_server, pn_server_selector))
    } else {
        None
    };

    let mut pn_server_runtime = None;
    let traffic_service = if start_pn_server {
        let pn_validator: p2p_frame::pn::PnConnectionValidatorRef =
            if let Some(client) = remote_control_client.as_ref() {
                VpnCmdPnConnectionValidator::new(client.clone())
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
        const MAX_USER_TRAFFIC_RETENTION_SECS: u64 = 30 * 24 * 60 * 60;
        let traffic_retention_secs = pn_config
            .report_interval_secs
            .checked_mul(2)
            .filter(|retention| *retention <= MAX_USER_TRAFFIC_RETENTION_SECS)
            .unwrap_or_else(|| {
                panic!(
                    "pn.report_interval_secs={} cannot produce a safe user traffic retention within 30 days",
                    pn_config.report_interval_secs
                )
            });
        pn_server.set_user_traffic_retention(std::time::Duration::from_secs(
            traffic_retention_secs,
        ));
        pn_server.start().await.unwrap();
        pn_server_runtime = Some(pn_server.clone());
        let traffic_service = if let Some((store_factory, _, _, _)) = control_runtime.as_ref() {
            PnTrafficService::new(store_factory.clone())
        } else {
            PnTrafficService::new_without_store()
        };
        traffic_service.set_node_traffic_source(pn_server.clone());
        traffic_service.set_proxy_upload_config(pn_traffic_upload_config);
        if let Some(client) = remote_control_client {
            traffic_service.set_remote_reporter(VpnCmdPnTrafficReporter::new(
                client,
                local_pn_server.clone(),
            ));
            traffic_service.start_remote_heartbeat(std::time::Duration::from_secs(
                pn_config.heartbeat_interval_secs,
            ));
            log::info!(
                "proxy node heartbeat and traffic report enabled heartbeat_interval_secs={} report_interval_secs={}",
                pn_config.heartbeat_interval_secs,
                pn_config.report_interval_secs,
            );
        } else if let Some((_, _, vpn_server, _)) = control_runtime.as_ref() {
            traffic_service.set_remote_reporter(LocalPnTrafficReporter::new(
                vpn_server.clone(),
                NodeId::from(local_id.as_slice()),
            ));
            log::info!(
                "process-local proxy traffic report enabled report_interval_secs={}",
                pn_config.report_interval_secs,
            );
        }
        traffic_service.start_background_flush(std::time::Duration::from_secs(
            pn_config.report_interval_secs,
        ));
        traffic_service
    } else {
        log::info!("default pn server disabled by config file {}", config_file);
        if let Some((store_factory, _, _, _)) = control_runtime.as_ref() {
            PnTrafficService::new(store_factory.clone())
        } else {
            PnTrafficService::new_without_store()
        }
    };
    traffic_service.start_node_traffic_cleanup();

    if let Some((_, user_manager, vpn_server, pn_server_selector)) = control_runtime {
        let sn_http_config = sn_http_config.expect("sn http config is parsed when sn is enabled");
        let http_config = HttpServerConfig::new(sn_http_config.ip, sn_http_config.port)
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

        tokio::select! {
            result = http_server.run() => result.unwrap(),
            result = tokio::signal::ctrl_c() => {
                if let Err(err) = result {
                    log::warn!("failed to wait for shutdown signal: {}", err);
                }
            }
        }
    } else if let Err(err) = tokio::signal::ctrl_c().await {
        log::warn!("failed to wait for shutdown signal: {}", err);
    }

    let drain_timeout = std::time::Duration::from_secs(
        pn_traffic_upload_config.shutdown_drain_secs,
    );
    let drain_deadline = std::time::Instant::now() + drain_timeout;
    let shutdown_status = traffic_service
        .shutdown_node_traffic(
            drain_deadline.saturating_duration_since(std::time::Instant::now()),
        )
        .await;
    if let Some(pn_server) = pn_server_runtime.as_ref() {
        pn_server.stop();
    }
    if !shutdown_status.is_success() {
        log::warn!(
            "node traffic graceful drain incomplete collector_exited={} final_collection_succeeded={} final_collection_error={:?} uploader_exited={} cleanup_exited={} queued_batches={} queued_records={} oldest_batch_id={:?} terminal_rejected_records={} crash_recovery=in-memory-only",
            shutdown_status.collector_exited,
            shutdown_status.final_collection_succeeded,
            shutdown_status.final_collection_error,
            shutdown_status.uploader_exited,
            shutdown_status.cleanup_exited,
            shutdown_status.queue.queued_batches,
            shutdown_status.queue.queued_records,
            shutdown_status.queue.oldest_batch_id,
            shutdown_status.queue.terminal_rejected_records,
        );
    }
}
