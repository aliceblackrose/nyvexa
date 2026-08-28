use anyhow::Result as AppResult;
use gloam_commands::prelude::*;

use crate::roles::revoke_fc_roles_for_user;

use super::{CommandData, finish, invoking_user_id};

#[command(
    description = "Remove your Lodestone character link and FC access",
    guild_only
)]
pub(crate) async fn unlink(ctx: Context<CommandData>) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let result = run(&ctx).await;
    finish(&ctx, result).await
}

async fn run(ctx: &Context<CommandData>) -> AppResult<String> {
    let user_id = invoking_user_id(ctx)?;
    revoke_fc_roles_for_user(ctx.rest(), &ctx.data().config, user_id).await?;
    ctx.data().store.unlink(user_id.get()).await?;
    Ok("Your Lodestone character link has been removed.".into())
}
