use main_error::MainError;
use tokio::fs::File;
use crate::config::Config;
use crate::trigger::TriggerManager;
use tokio::prelude::*;

mod config;
mod mdns;
mod trigger;

#[tokio::main]
async fn main() -> Result<(), MainError> {
    if let Some(path) = std::env::args().nth(1) {
        let mut file = File::open(path).await?;

        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let config: Config = serde_json::from_slice(&contents)?;
        let trigger_manager = TriggerManager::new(config);

        Ok(trigger_manager.run_triggers().await?)
    } else {
        println!("Usage {} config.json", std::env::args().next().unwrap());
        return Ok(());
    }
}