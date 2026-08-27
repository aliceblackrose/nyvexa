mod ping;

use gloam_commands::{Framework, Registration, commands};

#[derive(Debug, Default)]
pub(crate) struct CommandData;

pub(crate) fn framework(registration: Registration) -> gloam_commands::Result<Framework<CommandData>> {
    Framework::builder(CommandData)
        .commands(commands![ping::ping])
        .registration(registration)
        .build()
}
