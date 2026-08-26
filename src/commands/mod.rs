mod ping;

use std::error::Error;

use gloamwire::{
    RestClient,
    http::CreateApplicationCommand,
    model::{ApplicationId, Interaction, InteractionMessageData, InteractionResponse},
};

pub type CommandResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub async fn register_global(rest: &RestClient, application_id: ApplicationId) -> gloamwire::Result<()> {
    rest.create_global_application_command(application_id, &ping::definition())
        .await?;

    Ok(())
}

pub fn response(interaction: &Interaction) -> CommandResult<Option<InteractionResponse>> {
    let Some(command) = interaction.application_command_data()? else {
        return Ok(None);
    };

    let content = match command.name.as_str() {
        ping::NAME => ping::response(),
        _ => return Ok(None),
    };

    Ok(Some(InteractionResponse::message(
        InteractionMessageData::content(content),
    )))
}
