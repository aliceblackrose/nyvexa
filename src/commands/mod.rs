mod ping;

use std::error::Error;

use gloam_commands::{Framework, Registration, commands};
use gloamwire::model::{Interaction, InteractionMessageData, InteractionResponse};

#[derive(Debug, Default)]
pub struct CommandData;

pub type CommandResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub fn framework(registration: Registration) -> gloam_commands::Result<Framework<CommandData>> {
    Framework::builder(CommandData)
        .commands(commands![ping::ping])
        .registration(registration)
        .build()
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
