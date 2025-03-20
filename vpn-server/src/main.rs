use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use base58::ToBase58;
use config::builder::DefaultState;
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::p2p_identity::{P2pIdentity, P2pIdentityFactory};
use p2p_frame::sn::service::{create_sn_service, SnServiceConfig};
use p2p_frame::x509;
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use sfo_account::{hash_data, AccountServer, AccountStore, DefaultAccountManager};
use sfo_http::http_server::HttpServerConfig;
use sfo_http::openapi::OpenApiServer;
use sfo_http::openapi::utoipa;
use sfo_http::openapi::utoipa::OpenApi;
use sfo_http::tide_server::TideHttpServer;
use sfo_sql::sqlite::{SqlPool, SqliteJournalMode};
use vpn_frame::cmd_server::{CmdHandler};
use vpn_frame::server::{VpnCmdServer, VpnServer, VpnStoreFactory};
use vpn_frame::cmd_server::server::CmdServer;
use crate::api::Api;
use crate::sqlite_store_factory::{P2pSnCmdServer, SqliteStoreFactory};
use crate::user_store::{SqliteUserStore, User};

mod user_store;
mod sqlite_store_factory;
mod api;

#[derive(utoipa::OpenApi)]
#[openapi(paths(), components())]
struct ApiDoc;

#[tokio::main]
async fn main() {
    sfo_log::Logger::new("vpn-server")
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
        .get_matches();

    let config_file: String = matches.get_one::<String>("config").unwrap_or(&default_config).clone();
    let mut config = config::ConfigBuilder::<DefaultState>::default()
        .set_default("ip", "0.0.0.0").unwrap()
        .set_default("port", 3424).unwrap()
        .set_default("http.ip", "0.0.0.0").unwrap()
        .set_default("http.port", 3445).unwrap();
        // .set_default("jwt_key", "sdfasdgdfgsdfgsdfgsdfg").unwrap()
        // .set_default("admin.name", "wugren").unwrap()
        // .set_default("admin.password", "123456").unwrap();
    if Path::new(config_file.as_str()).exists() {
        config = config.add_source(config::File::from(Path::new(config_file.as_str())));
    }
    config = config.add_source(config::Environment::with_prefix("VPN").separator("_"));
    let config = config.build().unwrap();

    let ip = config.get_string("ip").unwrap();
    let port = config.get_int("port").unwrap() as u16;
    let http_ip = config.get_string("http.ip").unwrap();
    let http_port = config.get_int("http.port").unwrap() as u16;
    let admin_name = config.get_string("admin.name").unwrap();
    let admin_password = config.get_string("admin.password").unwrap();
    let jwt_key = config.get_string("jwt.key").unwrap().to_string();
    let data_dir = match config.get_string("data.dir") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            dirs::data_dir().unwrap().join("vpn-server")
        }
    };

    if !data_dir.exists() {
        tokio::fs::create_dir_all(data_dir.as_path()).await.unwrap();
    }
    let db_path = data_dir.join("vpn.db").to_string_lossy().to_string();
    let pool = SqlPool::open(db_path.as_str(), 300, Some(SqliteJournalMode::Wal)).await.unwrap();

    let user_store = SqliteUserStore::new(pool.clone());
    user_store.init_user_store().await.unwrap();

    let store_factory = Arc::new(SqliteStoreFactory::from_pool(pool));
    {
        let mut store = store_factory.get_vpn_store().await.unwrap();
        store.init_db().await.unwrap();
    }
    let eps = vec![Endpoint::from((Protocol::Quic, SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from_str(ip.as_str()).unwrap(), port))))];

    let identity_file = data_dir.join("identity");
    let identity = if identity_file.exists() {
        let data = tokio::fs::read(identity_file.as_path()).await.unwrap();
        let local_identity = X509IdentityFactory.create(&data).unwrap();
        local_identity
    } else {
        let local_identity = x509::generate_x509_identity(None).unwrap();
        let data = local_identity.get_encoded_identity().unwrap();
        tokio::fs::write(identity_file.as_path(), data).await.unwrap();
        Arc::new(local_identity)
    };
    let local_identity = identity.update_endpoints(eps);
    let local_id = local_identity.get_id();
    let sn_config = SnServiceConfig::new(
        local_identity,
        Arc::new(X509IdentityFactory),
        Arc::new(X509IdentityCertFactory)
    ).set_support_proxy(true);
    let sn_service = create_sn_service(sn_config).await;
    sn_service.start().await.unwrap();

    let vpn_server = VpnServer::new(Arc::new(P2pSnCmdServer::new(sn_service.clone())), store_factory.clone());
    let network_manager = vpn_server.network_manager().clone();

    if user_store.get_account(&admin_name).await.unwrap().is_none() {
        let network_id = network_manager.new_network_group().await.unwrap();
        let user = User {
            id: admin_name.clone(),
            password: hash_data(vec![admin_name.as_bytes(), admin_password.as_bytes()].as_slice()).to_base58(),
            network_id,
            server_id: local_id.to_string(),
        };
        user_store.add_account(&user).await.unwrap();
    }
    let user_manager = DefaultAccountManager::new(user_store, jwt_key.into_bytes());

    vpn_server.start();

    let http_config = HttpServerConfig::new(http_ip, http_port)
        .allow_any_header()
        .allow_any_origin()
        .allow_any_methods()
        .expose_any_header();
    let mut http_server = TideHttpServer::new(http_config);
    http_server.set_api_doc(ApiDoc::openapi());
    http_server.enable_api_doc(true);

    AccountServer::register_server(&mut http_server, user_manager.clone());
    Api::register_api(&mut http_server, user_manager.clone(), vpn_server.clone());

    http_server.run().await.unwrap();

    std::future::pending::<()>().await;
}
