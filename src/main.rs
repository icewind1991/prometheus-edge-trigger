use crate::config::Config;
use crate::trigger::TriggerManager;
use main_error::MainError;
use tokio::fs::File;
use tokio::prelude::*;

mod config;
mod mdns;
mod trigger;

#[tokio::main]
async fn main() -> Result<(), MainError> {
    env_logger::init();

    let mut args = std::env::args();
    let bin = args.next().unwrap();

    if let Some(path) = args.next() {
        let mut file = File::open(path).await?;

        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let config: Config = toml::from_slice(&contents)?;
        let trigger_manager = TriggerManager::new(config);

        Ok(trigger_manager.run_triggers().await?)
    } else {
        println!("Usage {} config.toml", bin);
        return Ok(());
    }
}
