use gloam_commands::prelude::*;

use super::CommandData;

pub const NAME: &str = "ping";

#[command(name = "ping", description = "Check whether Nyvexa is online")]
pub(crate) async fn ping(ctx: Context<CommandData>) -> Result<()> {
    ctx.reply(response()).await?;
    Ok(())
}

pub const fn response() -> &'static str {
    "Pong!"
}
