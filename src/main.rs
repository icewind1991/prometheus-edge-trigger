use main_error::MainError;
use tokio::fs::File;
use crate::config::Config;
use crate::trigger::TriggerManager;
use tokio::time::delay_for;
use std::time::Duration;
use tokio::prelude::*;

mod config;
mod mdns;
mod trigger;

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage {} config.json", args[0]);
        return Ok(());
    }

    let mut file = File::open("foo.txt").await?;

    let mut contents = vec![];
    file.read_to_end(&mut contents).await?;
    let config: Config = serde_json::from_slice(&contents)?;
    let trigger_manager = TriggerManager::new(config);

    loop {
        trigger_manager.poll_triggers().await?;

        delay_for(Duration::from_secs(60)).await;
    }
}