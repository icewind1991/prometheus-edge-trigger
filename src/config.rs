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
            Parameter::Value { value } => Ok(value.clone())
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
    pub delay: u64,
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