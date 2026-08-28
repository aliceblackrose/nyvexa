use anyhow::Result as AppResult;
use gloam_commands::prelude::*;

use crate::{
    clock::unix_timestamp, roles::apply_verified_roles_for_user, store::StoreError,
    verification::biography_contains_challenge,
};

use super::{CommandData, finish, invoking_user_id};

#[command(
    description = "Check the pending Lodestone verification code",
    guild_only
)]
pub(crate) async fn check(ctx: Context<CommandData>) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let result = run(&ctx).await;
    finish(&ctx, result).await
}

async fn run(ctx: &Context<CommandData>) -> AppResult<String> {
    let user_id = invoking_user_id(ctx)?;
    let Some(pending) = ctx.data().store.pending(user_id.get()).await? else {
        return Ok("No verification is pending. Run `/verify` first.".into());
    };

    let now = unix_timestamp()?;
    if pending.expires_at <= now {
        ctx.data().store.delete_pending(user_id.get()).await?;
        return Ok("That verification code expired. Run `/verify` again.".into());
    }

    let profile = ctx
        .data()
        .lodestone
        .character_profile(pending.character_id)
        .await?;
    if !biography_contains_challenge(&profile.biography, &pending.token_hash) {
        return Ok(
            "I don't see your verification code in the Lodestone self-introduction yet. Save the profile change and try `/check` again."
                .into(),
        );
    }

    if profile.free_company_id.as_deref() != Some(ctx.data().config.fc_id.as_str()) {
        return Ok(
            "Lodestone ownership is verified, but that character is not in this server's configured Free Company."
                .into(),
        );
    }

    let Some(fc_member) = ctx
        .data()
        .lodestone
        .find_fc_member(&ctx.data().config.fc_id, pending.character_id)
        .await?
    else {
        return Ok(
            "Lodestone ownership is verified, but the character is not present on the Free Company roster yet. Try again after Lodestone updates."
                .into(),
        );
    };

    match ctx
        .data()
        .store
        .link_verified(
            user_id.get(),
            pending.character_id,
            &pending.character_name,
            &pending.world,
            &fc_member.rank,
            now,
        )
        .await
    {
        Ok(()) => {}
        Err(StoreError::CharacterAlreadyLinked) => {
            return Ok(
                "That Lodestone character is already linked to another Discord account. Ask an administrator to resolve the existing link."
                    .into(),
            );
        }
        Err(error) => return Err(error.into()),
    }

    apply_verified_roles_for_user(ctx.rest(), &ctx.data().config, user_id, &fc_member.rank).await?;
    ctx.data().store.delete_pending(user_id.get()).await?;

    Ok(format!(
        "Verified **{}** on **{}**. Free Company membership confirmed with rank **{}**. Server access has been granted.",
        fc_member.name, fc_member.world, fc_member.rank
    ))
}
