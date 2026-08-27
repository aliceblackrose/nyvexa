mod commands;

use std::{env, error::Error};

use gloam_commands::Registration;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let token = env::var("DISCORD_TOKEN")?;
    let framework = commands::framework(Registration::Global)?;

    framework.run(token).await?;
    Ok(())
}
