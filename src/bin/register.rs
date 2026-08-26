use std::env;

use gloamwire::{RestClient, model::ApplicationId};

#[tokio::main]
async fn main() -> nyvexa::commands::CommandResult<()> {
    let token = env::var("DISCORD_TOKEN")?;
    let application_id = env::var("DISCORD_APPLICATION_ID")?.parse::<ApplicationId>()?;
    let rest = RestClient::new(token)?;

    nyvexa::commands::register_global(&rest, application_id).await?;
    println!("registered global application commands");

    Ok(())
}
