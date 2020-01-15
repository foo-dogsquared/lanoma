use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::Value;

use crate::consts;
use crate::error::Error;

const DEFAULT_FILES: &str = "*.tex";
const DEFAULT_CMD: &str = "pdflatex {{note}}";
const DEFAULT_NAME: &str = "New Student";

/// The configuration of a subject.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubjectConfig {
    #[serde(rename = "_files", default = "default_files")]
    pub files: Vec<String>,

    #[serde(default = "default_cmd")]
    pub command: String,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Default for SubjectConfig {
    fn default() -> Self {
        Self {
            files: vec![DEFAULT_FILES.to_string()],
            command: DEFAULT_CMD.to_string(),
            extra: HashMap::new(),
        }
    }
}

impl TryFrom<&Path> for SubjectConfig {
    type Error = Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let content = fs::read_to_string(path).map_err(Error::IoError)?;

        toml::from_str(&content).map_err(Error::TomlValueError)
    }
}

impl TryFrom<PathBuf> for SubjectConfig {
    type Error = Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let content = fs::read_to_string(path).map_err(Error::IoError)?;

        toml::from_str(&content).map_err(Error::TomlValueError)
    }
}

impl SubjectConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

fn default_files() -> Vec<String> {
    vec![DEFAULT_FILES.to_string()]
}

fn default_cmd() -> String {
    DEFAULT_CMD.to_string()
}

fn default_name() -> String {
    DEFAULT_NAME.to_string()
}

fn default_version() -> String {
    consts::APP_VERSION.into()
}

/// The configuration of a profile.
#[derive(Serialize, Deserialize, Clone)]
pub struct ProfileConfig {
    #[serde(default = "default_name")]
    pub name: String,

    #[serde(default = "default_version")]
    version: String,

    #[serde(default)]
    subject: SubjectConfig,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            subject: SubjectConfig::default(),
            name: default_name(),
            version: default_version(),
            extra: HashMap::new(),
        }
    }
}

impl TryFrom<&Path> for ProfileConfig {
    type Error = Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let content = fs::read_to_string(path).map_err(Error::IoError)?;

        toml::from_str(&content).map_err(Error::TomlValueError)
    }
}

impl TryFrom<PathBuf> for ProfileConfig {
    type Error = Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let content = fs::read_to_string(path).map_err(Error::IoError)?;

        toml::from_str(&content).map_err(Error::TomlValueError)
    }
}

impl ProfileConfig {
    pub fn new() -> Self {
        Self::default()
    }
}
