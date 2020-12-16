use crate::config::{
    Action, Condition, Config, Method, MqttConfig, Parameter, ParameterError, Trigger,
};
use err_derive::Error;
use futures_util::future::try_join_all;
use log::{error, info};
use main_error::MainError;
use prometheus_edge_detector::EdgeDetector;
use reqwest::Client;
use rumqttc::{AsyncClient, ClientError, Event, MqttOptions, Outgoing, QoS};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::time::delay_for;

pub struct TriggerManager {
    http_client: Client,
    mqtt_config: Option<MqttConfig>,
    edge_detector: EdgeDetector,
    triggers: Vec<Trigger>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
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
    #[error(display = "{}", _0)]
    Mqtt(#[error(source)] rumqttc::ClientError),
    #[error(display = "{}", _0)]
    Configuration(String),
}

impl TriggerManager {
    pub fn new(config: Config) -> TriggerManager {
        let edge_detector = EdgeDetector::new(&config.prometheus.url);

        TriggerManager {
            http_client: Client::new(),
            mqtt_config: config.mqtt,
            edge_detector,
            triggers: config.triggers,
        }
    }

    pub async fn run_triggers(&self) -> Result<(), MainError> {
        try_join_all(
            self.triggers
                .iter()
                .map(|trigger| self.run_trigger(trigger)),
        )
        .await?;

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
                    info!(
                        "[{}] Found edge, {}s ago, waiting {}s before triggering",
                        trigger.name, elapsed, wait
                    );
                    let wait_delay = Duration::from_secs(wait);
                    delay_for(wait_delay).await;

                    // verify that the previously found edge is still the most recent
                    match self.get_edge(&trigger.condition, delay).await {
                        Ok(Some(new_edge)) if new_edge == edge => {
                            info!("[{}] Edge still valid, triggering", trigger.name);
                            if let Err(e) = run_action(
                                &trigger.action,
                                &self.http_client,
                                self.mqtt_config.as_ref(),
                            )
                            .await
                            {
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
                    info!(
                        "[{}] No edge found, waiting {}s before looking for new edge",
                        trigger.name, delay
                    );
                    delay_for(delay_duration).await;
                }
                Err(e) => {
                    error!("[{}]: {}", trigger.name, e);
                    delay_for(error_delay).await;
                }
            }
        }
    }

    async fn get_edge(
        &self,
        condition: &Condition,
        delay: u64,
    ) -> Result<Option<u64>, TriggerError> {
        let query = interpolate_params(&condition.query, &condition.params).await?;
        Ok(self
            .edge_detector
            .get_last_edge(
                &query,
                condition.from,
                condition.to,
                Duration::from_secs(delay + 60),
            )
            .await?)
    }
}

async fn interpolate_option_params(
    input: &Option<String>,
    params: &HashMap<String, Parameter>,
) -> Result<Option<String>, ParameterError> {
    match input.as_ref() {
        Some(input) => Ok(Some(interpolate_params(input, params).await?)),
        None => Ok(None),
    }
}

async fn interpolate_params(
    input: &str,
    params: &HashMap<String, Parameter>,
) -> Result<String, ParameterError> {
    let futures = params
        .values()
        .map(|definition| Box::pin(definition.get_value()));

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

    let result = interpolate_params(
        "foo_$param",
        &hashmap! {
            "param".to_string() => Parameter::Value{value: "bar".to_string()}
        },
    )
    .await;
    assert_eq!("foo_bar".to_string(), result.unwrap());
}

async fn run_action(
    action: &Action,
    client: &Client,
    mqtt_config: Option<&MqttConfig>,
) -> Result<(), TriggerError> {
    let url = interpolate_option_params(&action.url, &action.params).await?;
    let topic = interpolate_option_params(&action.topic, &action.params).await?;
    let payload = interpolate_option_params(&action.payload, &action.params).await?;

    match (action.method, url, topic, payload) {
        (Method::Put, Some(url), _, _) => {
            client.put(&url).send().await?;
        }
        (Method::Post, Some(url), _, _) => {
            client.post(&url).send().await?;
        }
        (Method::Get, Some(url), _, _) => {
            client.get(&url).send().await?;
        }
        (Method::Mqtt, _, Some(topic), Some(payload)) => {
            if let Some(mqtt_config) = mqtt_config {
                send_mqtt_message(mqtt_config, topic, payload).await?;
            } else {
                return Err(TriggerError::Configuration(
                    "mqtt trigger configured, but no mqtt server configured".to_string(),
                ));
            }
        }
        _ => {}
    };

    Ok(())
}

async fn send_mqtt_message(
    config: &MqttConfig,
    topic: String,
    payload: String,
) -> Result<(), ClientError> {
    let hostname = hostname::get()
        .map(|os_str| os_str.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut mqtt_options = MqttOptions::new(
        format!("prometheus-edge-trigger-{}", hostname),
        config.host.as_str(),
        config.port.unwrap_or(1883),
    );
    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        mqtt_options.set_credentials(username, password);
    }

    let (mqtt_client, mut event_loop) = AsyncClient::new(mqtt_options, 10);
    mqtt_client
        .publish(topic, QoS::AtMostOnce, false, payload)
        .await?;
    mqtt_client.disconnect().await?;

    let _ = tokio::time::timeout(Duration::from_secs(1), async move {
        while let Ok(event) = event_loop.poll().await {
            if matches!(event, Event::Outgoing(Outgoing::Disconnect)) {
                break;
            }
        }
    })
    .await;
    Ok(())
}
