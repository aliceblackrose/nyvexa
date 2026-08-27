use std::env;

use gloam_commands::Registration;
use gloamwire::{RestClient, model::ApplicationId};

#[tokio::main]
async fn main() -> nyvexa::commands::CommandResult<()> {
    let token = env::var("DISCORD_TOKEN")?;
    let application_id = env::var("DISCORD_APPLICATION_ID")?.parse::<ApplicationId>()?;
    let rest = RestClient::new(token)?;
    let framework = nyvexa::commands::framework(Registration::Global)?;

    framework.synchronize_commands(&rest, application_id).await?;
    println!("registered global application commands");

    Ok(())
}
