use anyhow::Result as AppResult;
use gloam_commands::prelude::*;

use super::{CommandData, finish, invoking_user_id};

#[command(description = "Show your current Nyvexa verification status", guild_only)]
pub(crate) async fn status(ctx: Context<CommandData>) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let result = run(&ctx).await;
    finish(&ctx, result).await
}

async fn run(ctx: &Context<CommandData>) -> AppResult<String> {
    let user_id = invoking_user_id(ctx)?;
    Ok(match ctx.data().store.verified(user_id.get()).await? {
        Some(member) => {
            let membership = if member.missing_since.is_some() {
                "FC membership is currently pending revalidation"
            } else {
                "FC membership is active"
            };
            format!(
                "Linked to **{}** on **{}**. Last known FC rank: **{}**. {}.",
                member.character_name,
                member.world,
                member.fc_rank.as_deref().unwrap_or("Unknown"),
                membership
            )
        }
        None => "You are not verified. Run `/verify` to begin.".to_string(),
    })
}
