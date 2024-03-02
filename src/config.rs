use crate::mdns::resolve_mdns;
use secretfile::{load, SecretError};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParameterError {
    #[error("error while resolving mdns: {0}")]
    MdnsError(#[from] mdns::Error),
    #[error("requested mdns host not found")]
    MdnsHostNotFound,
    #[error("error reading file: {0}")]
    FilesystemError(#[from] std::io::Error),
    #[error("malformed service file: {0}")]
    Service(#[from] serde_json::Error),
    #[error("requested service not found")]
    ServiceNotFound,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum Parameter {
    Mdns {
        service: String,
        host: String,
    },
    Value {
        value: String,
    },
    Service {
        file: String,
        key: String,
        value: String,
    },
}

impl Parameter {
    pub async fn get_value(&self) -> Result<String, ParameterError> {
        match self {
            Parameter::Mdns { service, host } => match resolve_mdns(service, host).await? {
                Some(service) => Ok(service.to_string()),
                None => Err(ParameterError::MdnsHostNotFound),
            },
            Parameter::Value { value } => Ok(value.clone()),
            Parameter::Service { file, key, value } => {
                let content = tokio::fs::read(file).await?;
                let services: Vec<Service> = serde_json::from_slice(&content)?;
                services
                    .into_iter()
                    .find_map(|service| {
                        service
                            .labels
                            .get(key)
                            .filter(|val| *val == value)
                            .and_then(|_| service.targets.get(0))
                            .cloned()
                    })
                    .ok_or(ParameterError::ServiceNotFound)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Condition {
    #[serde(default)]
    pub params: HashMap<String, Parameter>,
    pub query: String,
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    pub method: Method,
    #[serde(default)]
    pub params: HashMap<String, Parameter>,
    pub url: Option<String>,
    pub topic: Option<String>,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub prometheus: PrometheusConfig,
    pub mqtt: Option<MqttConfig>,
    #[serde(rename = "trigger")]
    pub triggers: Vec<Trigger>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrometheusConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawMqttConfig")]
pub struct MqttConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMqttConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    #[serde(flatten)]
    pub password: Option<MqttPassword>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum MqttPassword {
    Raw { password: String },
    File { password_file: String },
}

impl TryFrom<RawMqttConfig> for MqttConfig {
    type Error = SecretError;

    fn try_from(value: RawMqttConfig) -> Result<Self, Self::Error> {
        let password = match value.password {
            Some(MqttPassword::Raw { password }) => Some(password),
            Some(MqttPassword::File { password_file }) => Some(load(&password_file)?),
            None => None,
        };
        Ok(MqttConfig {
            host: value.host,
            port: value.port,
            username: value.username,
            password,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trigger {
    pub name: String,
    pub delay: u64,
    pub condition: Condition,
    pub action: Action,
}

#[derive(Debug, Clone, Deserialize, Copy)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Put,
    Post,
    Mqtt,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Service {
    targets: Vec<String>,
    labels: HashMap<String, String>,
}
