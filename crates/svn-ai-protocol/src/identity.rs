use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

const DIGEST_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest([u8; DIGEST_LEN]);

impl Digest {
    pub const fn new(bytes: [u8; DIGEST_LEN]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; DIGEST_LEN] {
        &self.0
    }

    pub fn from_hex(value: &str) -> Result<Self, DigestError> {
        let mut bytes = [0_u8; DIGEST_LEN];
        hex::decode_to_slice(value, &mut bytes).map_err(|_| DigestError)?;
        Ok(Self(bytes))
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

impl Serialize for Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_hex(&value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("digest must contain exactly 64 hexadecimal characters")]
pub struct DigestError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

impl RepoPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for RepoPath {
    type Error = RepoPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        validate_repo_path(value)?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for RepoPath {
    type Error = RepoPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_repo_path(&value)?;
        Ok(Self(value))
    }
}

impl From<RepoPath> for String {
    fn from(value: RepoPath) -> Self {
        value.0
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RepoPathError {
    #[error("repository path cannot be empty")]
    Empty,
    #[error("repository path must be relative and cannot contain a drive prefix")]
    Absolute,
    #[error("repository path must use forward slashes")]
    Backslash,
    #[error("repository path contains a forbidden component")]
    ForbiddenComponent,
    #[error("repository path contains a NUL byte")]
    Nul,
}

fn validate_repo_path(value: &str) -> Result<(), RepoPathError> {
    if value.is_empty() {
        return Err(RepoPathError::Empty);
    }
    if value.contains('\0') {
        return Err(RepoPathError::Nul);
    }
    if value.starts_with('/') || value.starts_with('\\') {
        return Err(RepoPathError::Absolute);
    }
    if value.as_bytes().get(1) == Some(&b':') && value.as_bytes()[0].is_ascii_alphabetic() {
        return Err(RepoPathError::Absolute);
    }
    if value.contains('\\') {
        return Err(RepoPathError::Backslash);
    }
    if value
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(RepoPathError::ForbiddenComponent);
    }
    Ok(())
}
