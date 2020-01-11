use main_error::MainError;
use std::collections::HashMap;
use crate::config::{Parameter, ParameterError, Trigger, Config, Action};
use prometheus_edge_detector::EdgeDetector;
use futures_util::future::try_join_all;
use std::time::{Duration, SystemTime};
use reqwest::Client;


pub struct TriggerManager {
    http_client: Client,
    edge_detector: EdgeDetector,
    triggers: Vec<Trigger>,
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

    pub async fn poll_triggers(&self) -> Result<(), MainError> {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

        for trigger in &self.triggers {
            let delay = trigger.action.delay;
            let query = interpolate_params(&trigger.trigger.query, &trigger.trigger.params).await?;
            let edge = self.edge_detector.get_last_edge(&query, trigger.trigger.from, trigger.trigger.to, Duration::from_secs(delay * 2)).await?;

            if let Some(edge) = edge {
                let edge_from_now = now - edge;
                if edge_from_now > delay && (edge_from_now - delay) < 60 {
                    run_action(&trigger.action, &self.http_client).await?;
                }
            }
        }

        Ok(())
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