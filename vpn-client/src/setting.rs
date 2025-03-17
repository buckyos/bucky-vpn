use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use vpn_frame::errors::{into_vpn_err, VpnErrorCode, VpnResult};

pub struct Setting {
    config_file: PathBuf,
    config: Mutex<HashMap<String, serde_json::Value>>,
}

impl Setting {
    pub async fn load(config_file: &Path) -> VpnResult<Setting> {
        let config = if config_file.exists() {
            let content = tokio::fs::read_to_string(config_file).await
                .map_err(into_vpn_err!(VpnErrorCode::Failed, "read config file {} failed", config_file.to_string_lossy().to_string()))?;
            toml::from_str(content.as_str()).map_err(into_vpn_err!(VpnErrorCode::Failed, "parse config file {} failed", config_file.to_string_lossy().to_string()))?
        } else {
            HashMap::new()
        };
        Ok(Self {
            config_file: config_file.to_path_buf(),
            config: Mutex::new(config),
        })
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let config = self.config.lock().unwrap();
        if let Some(value) = config.get(key) {
            match serde_json::from_value(value.to_owned()) {
                Ok(value) => Some(value),
                Err(_) => {
                    log::error!("parse config {} failed", key);
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn set<T: serde::ser::Serialize>(&self, key: &str, value: T) -> VpnResult<()> {
        let mut config = self.config.lock().unwrap();
        let value = serde_json::to_value(value).map_err(into_vpn_err!(VpnErrorCode::Failed, "serialize config {} failed", key))?;
        config.insert(key.to_string(), value);
        Ok(())
    }

    pub async fn save(&self) -> VpnResult<()> {
        let toml_string = {
            let config = self.config.lock().unwrap();
            toml::to_string(config.deref()).map_err(into_vpn_err!(VpnErrorCode::Failed, "toml to string failed"))?
        };
        tokio::fs::write(&self.config_file, toml_string).await
            .map_err(into_vpn_err!(VpnErrorCode::Failed, "write config file {} failed", self.config_file.to_string_lossy().to_string()))?;
        Ok(())
    }
}
