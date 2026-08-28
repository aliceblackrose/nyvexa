use anyhow::Result as AppResult;
use gloam_commands::prelude::*;

use crate::{
    clock::unix_timestamp,
    lodestone::LodestoneError,
    store::NewPendingVerification,
    verification::{generate_challenge, hash_token},
};

use super::{CommandData, finish, invoking_user_id};

#[command(
    description = "Start Lodestone and Free Company verification",
    guild_only
)]
pub(crate) async fn verify(
    ctx: Context<CommandData>,
    #[description = "Exact FFXIV character name"]
    #[min_length = 1]
    #[max_length = 64]
    character: String,
    #[description = "Character World, for example Cactuar"]
    #[min_length = 1]
    #[max_length = 64]
    world: String,
) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let result = run(&ctx, &character, &world).await;
    finish(&ctx, result).await
}

async fn run(ctx: &Context<CommandData>, character: &str, world: &str) -> AppResult<String> {
    let user_id = invoking_user_id(ctx)?;
    let character = match ctx.data().lodestone.find_character(character, world).await {
        Ok(character) => character,
        Err(LodestoneError::CharacterNotFound) => {
            return Ok("I couldn't find that character on Lodestone.".into());
        }
        Err(LodestoneError::AmbiguousCharacter) => {
            return Ok(
                "Lodestone returned more than one exact match. Check the character name and World."
                    .into(),
            );
        }
        Err(error) => return Err(error.into()),
    };

    let challenge = generate_challenge();
    let challenge_hash = hash_token(&challenge);
    let now = unix_timestamp()?;
    ctx.data()
        .store
        .put_pending(NewPendingVerification {
            discord_user_id: user_id.get(),
            character_id: character.id,
            character_name: &character.name,
            world: &character.world,
            token_hash: &challenge_hash,
            created_at: now,
            expires_at: now + ctx.data().config.challenge_ttl_secs as i64,
        })
        .await?;

    Ok(format!(
        "Found **{}** on **{}**.\n\nPut this one-time code anywhere in that character's Lodestone self-introduction:\n`{}`\n\nThen run `/check`. The code expires in {} minutes. You can remove it after verification succeeds.",
        character.name,
        character.world,
        challenge,
        ctx.data().config.challenge_ttl_secs / 60
    ))
}
