use main_error::MainError;
use std::collections::HashMap;
use crate::config::{Parameter, ParameterError, Trigger, Config, Action};
use prometheus_edge_detector::EdgeDetector;
use futures_util::future::try_join_all;
use std::time::{Duration, SystemTime};
use reqwest::Client;
use tokio::time::delay_for;

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

impl TriggerManager {
    pub fn new(config: Config) -> TriggerManager {
        let edge_detector = EdgeDetector::new(&config.prometheus_url);

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

    pub async fn run_trigger(&self, trigger: &Trigger) -> Result<(), MainError> {
        let delay = trigger.action.delay;
        let delay_duration = Duration::from_secs(delay);
        loop {
            let query = interpolate_params(&trigger.trigger.query, &trigger.trigger.params).await?;
            let edge = self.edge_detector.get_last_edge(&query, trigger.trigger.from, trigger.trigger.to, Duration::from_secs(delay + 60)).await?;
            if let Some(edge) = edge {
                let elapsed = since(edge);
                let wait = delay.saturating_sub(elapsed);
                println!("Found edge, {}s ago, waiting {}s before triggering", elapsed, wait);
                let wait_delay = Duration::from_secs(wait);
                delay_for(wait_delay).await;

                // verify that the previously found edge is still the most recent
                let new_edge = self.edge_detector.get_last_edge(&query, trigger.trigger.from, trigger.trigger.to, Duration::from_secs(delay + 60)).await?;
                if new_edge == Some(edge) {
                    println!("Edge still valid, triggering");
                    run_action(&trigger.action, &self.http_client).await?;
                    delay_for(delay_duration).await;
                } else {
                    println!("Edge no longer value");
                }
            } else {
                println!("No edge found, waiting {}s before looking for new edge", delay);
                delay_for(delay_duration).await;
            }
        }
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

async fn run_action(action: &Action, client: &Client) -> Result<(), MainError> {
    let url = interpolate_params(&action.url, &action.params).await?;

    let req = match action.method.to_ascii_lowercase().as_str() {
        "put" => client.put(&url),
        "post" => client.post(&url),
        "get" => client.get(&url),
        _ => unimplemented!()
    };
    req.send().await?;

    Ok(())
}