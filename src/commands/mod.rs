mod ping;

use std::error::Error;

use gloamwire::{
    RestClient,
    http::{CreateApplicationCommand, CreateInteractionResponseQuery},
    model::{ApplicationId, Interaction, InteractionMessageData, InteractionResponse},
};

pub type CommandResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub async fn register_global(rest: &RestClient, application_id: ApplicationId) -> gloamwire::Result<()> {
    rest.create_global_application_command(application_id, &ping::definition())
        .await?;

    Ok(())
}

pub async fn handle(rest: &RestClient, interaction: &Interaction) -> CommandResult<()> {
    let Some(command) = interaction.application_command_data()? else {
        return Ok(());
    };

    let response = match command.name.as_str() {
        ping::NAME => ping::response(),
        _ => return Ok(()),
    };

    rest.create_interaction_response(
        interaction.id,
        &interaction.token,
        &InteractionResponse::message(InteractionMessageData::content(response)),
        &CreateInteractionResponseQuery::default(),
    )
    .await?;

    Ok(())
}
