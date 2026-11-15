use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::error::ConfigError;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
pub struct MacBackup {
    pub iface: String,
    pub original: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
pub struct HostnameBackup {
    pub original: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
pub struct DnsBackup {
    pub resolv_contents: String,
    pub iface: Option<String>,
    pub kind: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct State {
    pub mac: Option<MacBackup>,
    pub hostname: Option<HostnameBackup>,
    pub dns: Option<DnsBackup>,
}

impl State {
    pub fn from_json(text: &str) -> Result<State, ConfigError> {
        serde_json::from_str(text).map_err(ConfigError::ParseFailed)
    }

    pub fn to_json(&self) -> Result<String, ConfigError> {
        serde_json::to_string_pretty(self).map_err(ConfigError::ParseFailed)
    }

    pub fn is_empty(&self) -> bool {
        self.mac.is_none() && self.hostname.is_none() && self.dns.is_none()
    }
}

pub fn state_path() -> PathBuf {
    if nix::unistd::geteuid().is_root() {
        return PathBuf::from("/var/lib/privcrab/state.json");
    }
    match dirs::data_local_dir() {
        Some(base) => base.join("privcrab").join("state.json"),
        None => {
            eprintln!("[!] data directory not found, using /tmp/privcrab-state.json");
            PathBuf::from("/tmp/privcrab-state.json")
        }
    }
}