use main_error::MainError;
use std::collections::HashMap;
use crate::config::{Parameter, ParameterError, Trigger, Config, Action, Method, Condition};
use prometheus_edge_detector::EdgeDetector;
use futures_util::future::try_join_all;
use std::time::{Duration, SystemTime};
use reqwest::Client;
use tokio::time::delay_for;
use log::{info, error};
use err_derive::Error;

pub struct TriggerManager {
    http_client: Client,
    edge_detector: EdgeDetector,
    triggers: Vec<Trigger>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
}

fn since(time: u64) -> u64 {
    now().saturating_sub(time)
}

#[derive(Debug, Error)]
pub enum TriggerError {
    #[error(display = "{}", _0)]
    Parameter(#[error(source)] ParameterError),
    #[error(display = "{}", _0)]
    Edge(#[error(source)] prometheus_edge_detector::Error),
    #[error(display = "{}", _0)]
    Network(#[error(source)] reqwest::Error),
}

impl TriggerManager {
    pub fn new(config: Config) -> TriggerManager {
        let edge_detector = EdgeDetector::new(&config.prometheus.url);

        TriggerManager {
            http_client: Client::new(),
            edge_detector,
            triggers: config.triggers,
        }
    }

    pub async fn run_triggers(&self) -> Result<(), MainError> {
        try_join_all(self.triggers.iter().map(|trigger| {
            self.run_trigger(trigger)
        })).await?;

        Ok(())
    }

    async fn run_trigger(&self, trigger: &Trigger) -> Result<(), MainError> {
        let delay = trigger.delay;
        let delay_duration = Duration::from_secs(delay);
        let error_delay = Duration::from_secs(15);
        loop {
            match self.get_edge(&trigger.condition, delay).await {
                Ok(Some(edge)) => {
                    let elapsed = since(edge);
                    let wait = delay.saturating_sub(elapsed);
                    info!("[{}] Found edge, {}s ago, waiting {}s before triggering", trigger.name, elapsed, wait);
                    let wait_delay = Duration::from_secs(wait);
                    delay_for(wait_delay).await;

                    // verify that the previously found edge is still the most recent
                    match self.get_edge(&trigger.condition, delay).await {
                        Ok(Some(new_edge)) if new_edge == edge => {
                            info!("[{}] Edge still valid, triggering", trigger.name);
                            if let Err(e) = run_action(&trigger.action, &self.http_client).await {
                                error!("[{}]: {}", trigger.name, e);
                            }
                            delay_for(delay_duration).await;
                        }
                        Err(e) => {
                            error!("[{}]: {}", trigger.name, e);
                            delay_for(error_delay).await;
                        }
                        _ => {
                            info!("[{}] Edge no longer valid", trigger.name);
                        }
                    }
                }
                Ok(None) => {
                    info!("[{}] No edge found, waiting {}s before looking for new edge", trigger.name, delay);
                    delay_for(delay_duration).await;
                }
                Err(e) => {
                    error!("[{}]: {}", trigger.name, e);
                    delay_for(error_delay).await;
                }
            }
        }
    }

    async fn get_edge(&self, condition: &Condition, delay: u64) -> Result<Option<u64>, TriggerError> {
        let query = interpolate_params(&condition.query, &condition.params).await?;
        Ok(self.edge_detector.get_last_edge(&query, condition.from, condition.to, Duration::from_secs(delay + 60)).await?)
    }
}

async fn interpolate_params(input: &str, params: &HashMap<String, Parameter>) -> Result<String, ParameterError> {
    let futures = params.values().map(|definition| {
        Box::pin(definition.get_value())
    });

    let resolved_params: Vec<String> = try_join_all(futures).await?;
    let mut result = input.to_string();

    for (name, value) in params.keys().zip(resolved_params.into_iter()) {
        result = result.replace(&format!("${}", name), &value);
    }

    Ok(result)
}

#[tokio::test]
async fn test_interpolate() {
    use maplit::hashmap;

    let result = interpolate_params("foo_$param", &hashmap! {
        "param".to_string() => Parameter::Value{value: "bar".to_string()}
    }).await;
    assert_eq!("foo_bar".to_string(), result.unwrap());
}


async fn run_action(action: &Action, client: &Client) -> Result<(), TriggerError> {
    let url = interpolate_params(&action.url, &action.params).await?;

    let req = match action.method {
        Method::Put => client.put(&url),
        Method::Post => client.post(&url),
        Method::Get => client.get(&url),
    };
    req.send().await?;

    Ok(())
}