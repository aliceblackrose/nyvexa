use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result};
use gloamwire::{
    RestClient,
    gateway::GuildMemberAddEvent,
    model::{GuildId, GuildMember, RoleId, UserId},
};
use tracing::{info, warn};

use crate::{
    clock::unix_timestamp, commands::CommandData, config::Config, lodestone::FreeCompanyMember,
};

const AUDIT_REASON: &str = "Nyvexa Free Company verification";

pub(crate) async fn handle_member_add(
    rest: &RestClient,
    data: &CommandData,
    event: GuildMemberAddEvent,
) -> Result<()> {
    if event.guild_id.get() != data.config.guild_id {
        return Ok(());
    }

    let mut member = event.member;
    let Some(user) = member.user.as_ref() else {
        return Ok(());
    };
    if user.bot.unwrap_or(false) {
        return Ok(());
    }
    let user_id = user.id;

    match data.store.verified(user_id.get()).await? {
        Some(verified) if verified.missing_since.is_none() => {
            if let Some(rank) = verified.fc_rank.as_deref() {
                if let Err(error) =
                    apply_verified_roles(rest, &data.config, user_id, &mut member, rank).await
                {
                    warn!(?error, user = %user_id, "failed to restore verified roles");
                }
            }
        }
        Some(_) => {
            if let Err(error) = add_role_if_missing(
                rest,
                GuildId::new(data.config.guild_id),
                user_id,
                &mut member,
                RoleId::new(data.config.unverified_role_id),
            )
            .await
            {
                warn!(?error, user = %user_id, "failed to restore restricted role");
            }
        }
        None => {
            if let Err(error) = add_role_if_missing(
                rest,
                GuildId::new(data.config.guild_id),
                user_id,
                &mut member,
                RoleId::new(data.config.unverified_role_id),
            )
            .await
            {
                warn!(?error, user = %user_id, "failed to add unverified role");
            }
        }
    }

    Ok(())
}

pub(crate) async fn apply_verified_roles_for_user(
    rest: &RestClient,
    config: &Config,
    user_id: UserId,
    rank: &str,
) -> Result<()> {
    let guild_id = GuildId::new(config.guild_id);
    let mut member = rest
        .get_guild_member(guild_id, user_id)
        .await
        .context("verified user is not a guild member")?;
    apply_verified_roles(rest, config, user_id, &mut member, rank).await
}

pub(crate) async fn revoke_fc_roles_for_user(
    rest: &RestClient,
    config: &Config,
    user_id: UserId,
) -> Result<()> {
    let guild_id = GuildId::new(config.guild_id);
    let Ok(mut member) = rest.get_guild_member(guild_id, user_id).await else {
        return Ok(());
    };
    revoke_fc_roles(rest, config, user_id, &mut member).await
}

async fn apply_verified_roles(
    rest: &RestClient,
    config: &Config,
    user_id: UserId,
    member: &mut GuildMember,
    rank: &str,
) -> Result<()> {
    let guild_id = GuildId::new(config.guild_id);
    remove_role_if_present(
        rest,
        guild_id,
        user_id,
        member,
        RoleId::new(config.unverified_role_id),
    )
    .await?;
    add_role_if_missing(
        rest,
        guild_id,
        user_id,
        member,
        RoleId::new(config.member_role_id),
    )
    .await?;

    for role_id in config.rank_roles.values().copied() {
        let role = RoleId::new(role_id);
        if Some(role_id) == config.rank_role_for(rank) {
            add_role_if_missing(rest, guild_id, user_id, member, role).await?;
        } else {
            remove_role_if_present(rest, guild_id, user_id, member, role).await?;
        }
    }
    Ok(())
}

async fn revoke_fc_roles(
    rest: &RestClient,
    config: &Config,
    user_id: UserId,
    member: &mut GuildMember,
) -> Result<()> {
    let guild_id = GuildId::new(config.guild_id);
    remove_role_if_present(
        rest,
        guild_id,
        user_id,
        member,
        RoleId::new(config.member_role_id),
    )
    .await?;
    for role_id in config.rank_roles.values().copied() {
        remove_role_if_present(rest, guild_id, user_id, member, RoleId::new(role_id)).await?;
    }
    add_role_if_missing(
        rest,
        guild_id,
        user_id,
        member,
        RoleId::new(config.unverified_role_id),
    )
    .await?;
    Ok(())
}

async fn add_role_if_missing(
    rest: &RestClient,
    guild_id: GuildId,
    user_id: UserId,
    member: &mut GuildMember,
    role: RoleId,
) -> Result<()> {
    if !member.roles.contains(&role) {
        rest.add_guild_member_role(guild_id, user_id, role, Some(AUDIT_REASON))
            .await?;
        member.roles.push(role);
    }
    Ok(())
}

async fn remove_role_if_present(
    rest: &RestClient,
    guild_id: GuildId,
    user_id: UserId,
    member: &mut GuildMember,
    role: RoleId,
) -> Result<()> {
    if member.roles.contains(&role) {
        rest.remove_guild_member_role(guild_id, user_id, role, Some(AUDIT_REASON))
            .await?;
        member.roles.retain(|existing| *existing != role);
    }
    Ok(())
}

pub(crate) async fn run_membership_sync_loop(rest: RestClient, data: CommandData) {
    loop {
        if let Err(error) = sync_all_members(&rest, &data).await {
            warn!(?error, "membership synchronization failed");
        }
        tokio::time::sleep(Duration::from_secs(data.config.sync_interval_secs)).await;
    }
}

async fn sync_all_members(rest: &RestClient, data: &CommandData) -> Result<()> {
    let roster = data.lodestone.fc_roster(&data.config.fc_id).await?;
    let roster: HashMap<u64, FreeCompanyMember> = roster
        .into_iter()
        .map(|member| (member.character_id, member))
        .collect();
    let verified = data.store.all_verified().await?;
    let now = unix_timestamp()?;
    let guild_id = GuildId::new(data.config.guild_id);

    info!(
        verified = verified.len(),
        roster = roster.len(),
        "synchronizing FC membership"
    );

    for record in verified {
        let user_id = UserId::new(record.discord_user_id);
        let mut discord_member = match rest.get_guild_member(guild_id, user_id).await {
            Ok(member) => member,
            Err(error) => {
                warn!(
                    ?error,
                    user = record.discord_user_id,
                    "verified user is no longer in Discord guild"
                );
                continue;
            }
        };

        if let Some(fc_member) = roster.get(&record.character_id) {
            data.store
                .mark_present(record.discord_user_id, &fc_member.rank, now)
                .await?;
            if let Err(error) = apply_verified_roles(
                rest,
                &data.config,
                user_id,
                &mut discord_member,
                &fc_member.rank,
            )
            .await
            {
                warn!(
                    ?error,
                    user = record.discord_user_id,
                    "failed to synchronize verified roles"
                );
            }
            continue;
        }

        let missing_since = data.store.mark_missing(record.discord_user_id, now).await?;
        if now.saturating_sub(missing_since) < data.config.membership_grace_secs as i64 {
            warn!(
                user = record.discord_user_id,
                character = %record.character_name,
                "character missing from FC roster; grace period active"
            );
            continue;
        }

        if let Err(error) = revoke_fc_roles(rest, &data.config, user_id, &mut discord_member).await
        {
            warn!(
                ?error,
                user = record.discord_user_id,
                "failed to revoke stale FC roles"
            );
        }
    }
    Ok(())
}
