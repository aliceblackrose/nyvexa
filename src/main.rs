mod commands;
mod config;

use gloam_commands::Registration;

use config::Config;

#[tokio::main]
async fn main() -> commands::CommandResult<()> {
    let config = Config::from_env()?;
    let framework = commands::framework(Registration::Global)?;

    framework.run(config.discord_token).await?;
    Ok(())
}
