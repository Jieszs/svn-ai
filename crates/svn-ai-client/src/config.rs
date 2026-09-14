use std::{fmt, fs, path::PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

const CONFIG_FILE: &str = "config.json";

#[derive(Clone, PartialEq, Eq)]
pub struct FingerprintSecret([u8; 32]);

impl FingerprintSecret {
    pub fn parse(value: &str) -> Result<Self, ConfigError> {
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(value, &mut bytes)
            .map_err(|_| ConfigError::InvalidFingerprintSecret)?;
        Ok(Self(bytes))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for FingerprintSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FingerprintSecret([REDACTED])")
    }
}

impl Serialize for FingerprintSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for FingerprintSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientConfig {
    pub svn_username: String,
    pub fingerprint_secret: FingerprintSecret,
    pub svn_executable: PathBuf,
}

impl ClientConfig {
    pub fn load(home: &std::path::Path) -> Result<Self, ConfigError> {
        let content =
            fs::read(home.join(CONFIG_FILE)).map_err(|error| ConfigError::Io(error.to_string()))?;
        serde_json::from_slice(&content).map_err(|error| ConfigError::Json(error.to_string()))
    }

    pub fn save(&self, home: &std::path::Path) -> Result<(), ConfigError> {
        fs::create_dir_all(home).map_err(|error| ConfigError::Io(error.to_string()))?;
        let content = serde_json::to_vec_pretty(self)
            .map_err(|error| ConfigError::Json(error.to_string()))?;
        fs::write(home.join(CONFIG_FILE), content)
            .map_err(|error| ConfigError::Io(error.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    #[error("fingerprint secret must contain exactly 64 hexadecimal characters")]
    InvalidFingerprintSecret,
    #[error("configuration I/O failed: {0}")]
    Io(String),
    #[error("configuration JSON is invalid: {0}")]
    Json(String),
}
