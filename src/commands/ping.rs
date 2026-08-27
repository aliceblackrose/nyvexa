use gloam_commands::prelude::*;

use super::CommandData;

#[command(description = "Check whether Nyvexa is online")]
pub(crate) async fn ping(ctx: Context<CommandData>) -> Result<()> {
    ctx.reply("Pong!").await
}
