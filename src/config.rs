use serde::Deserialize;
use err_derive::Error;
use crate::mdns::resolve_mdns;
use std::collections::HashMap;

#[derive(Debug, Error)]
pub enum ParameterError {
    #[error(display = "error while resolving mdns: {}", _0)]
    MdnsError(#[error(source)] mdns::Error),
    #[error(display = "requested mdns host not found")]
    MdnsHostNotFound,
    #[error(display = "error reading file: {}", _0)]
    FilesystemError(#[error(source)] std::io::Error),
    #[error(display = "malformed service file: {}", _0)]
    Service(#[error(source)] serde_json::Error),
    #[error(display = "requested service not found")]
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
            Parameter::Mdns {
                service,
                host
            } => {
                match resolve_mdns(service, host).await? {
                    Some(service) => Ok(format!("{}:{}", service.addr, service.port)),
                    None => Err(ParameterError::MdnsHostNotFound)
                }
            }
            Parameter::Value { value } => Ok(value.clone()),
            Parameter::Service {
                file, key, value
            } => {
                let content = tokio::fs::read(file).await?;
                let services: Vec<Service> = serde_json::from_slice(&content)?;
                services.into_iter().find_map(|service| {
                    service.labels.get(key)
                        .filter(|val| *val == value)
                        .and_then(|_| service.targets.get(0))
                        .cloned()
                }).ok_or(ParameterError::ServiceNotFound)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Condition {
    pub params: HashMap<String, Parameter>,
    pub query: String,
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    pub method: Method,
    pub params: HashMap<String, Parameter>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub prometheus: PrometheusConfig,
    #[serde(rename = "trigger")]
    pub triggers: Vec<Trigger>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrometheusConfig {
    pub url: String
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trigger {
    pub name: String,
    pub delay: u64,
    pub condition: Condition,
    pub action: Action,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Put,
    Post,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Service {
    targets: Vec<String>,
    labels: HashMap<String, String>,
}