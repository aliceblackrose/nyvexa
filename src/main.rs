mod bot;
mod commands;
mod config;

use bot::Bot;
use config::Config;

#[tokio::main]
async fn main() -> commands::CommandResult<()> {
    let config = Config::from_env()?;
    let mut bot = Bot::connect(config).await?;
    bot.run().await
}
