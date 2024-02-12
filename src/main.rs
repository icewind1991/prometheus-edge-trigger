use crate::config::Config;
use crate::trigger::TriggerManager;
use main_error::MainError;
use std::fs::read_to_string;

mod config;
mod mdns;
mod trigger;

#[tokio::main]
async fn main() -> Result<(), MainError> {
    env_logger::init();

    let mut args = std::env::args();
    let bin = args.next().unwrap();

    if let Some(path) = args.next() {
        let contents = read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        let trigger_manager = TriggerManager::new(config);

        Ok(trigger_manager.run_triggers().await?)
    } else {
        println!("Usage {} config.toml", bin);
        return Ok(());
    }
}
