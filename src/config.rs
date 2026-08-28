use std::{
    collections::{BTreeMap, HashSet},
    env,
};

use anyhow::{Context, Result, bail};

#[derive(Clone)]
pub(crate) struct Config {
    pub(crate) discord_token: String,
    pub(crate) guild_id: u64,
    pub(crate) fc_id: String,
    pub(crate) member_role_id: u64,
    pub(crate) unverified_role_id: u64,
    pub(crate) lodestone_region: String,
    pub(crate) rank_roles: BTreeMap<String, u64>,
    pub(crate) database_url: String,
    pub(crate) sync_interval_secs: u64,
    pub(crate) membership_grace_secs: u64,
    pub(crate) challenge_ttl_secs: u64,
}

impl Config {
    pub(crate) fn from_env() -> Result<Self> {
        let lodestone_region = env::var("NYVEXA_LODESTONE_REGION").unwrap_or_else(|_| "na".into());
        if !matches!(lodestone_region.as_str(), "na" | "eu" | "jp" | "fr" | "de") {
            bail!("NYVEXA_LODESTONE_REGION must be one of: na, eu, jp, fr, de");
        }

        let rank_roles = env::var("NYVEXA_RANK_ROLES")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| serde_json::from_str(&value).context("invalid NYVEXA_RANK_ROLES JSON"))
            .transpose()?
            .unwrap_or_default();

        let config = Self {
            discord_token: required("DISCORD_TOKEN")?,
            guild_id: parse_required("NYVEXA_GUILD_ID")?,
            fc_id: required("NYVEXA_FC_ID")?,
            member_role_id: parse_required("NYVEXA_MEMBER_ROLE_ID")?,
            unverified_role_id: parse_required("NYVEXA_UNVERIFIED_ROLE_ID")?,
            lodestone_region,
            rank_roles,
            database_url: env::var("NYVEXA_DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://nyvexa.db?mode=rwc".into()),
            sync_interval_secs: parse_optional("NYVEXA_SYNC_INTERVAL_SECS", 900)?,
            membership_grace_secs: parse_optional("NYVEXA_MEMBERSHIP_GRACE_SECS", 43_200)?,
            challenge_ttl_secs: parse_optional("NYVEXA_CHALLENGE_TTL_SECS", 1_800)?,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.fc_id.is_empty() || !self.fc_id.chars().all(|ch| ch.is_ascii_digit()) {
            bail!("NYVEXA_FC_ID must contain only decimal digits");
        }
        if self.member_role_id == self.unverified_role_id {
            bail!("NYVEXA_MEMBER_ROLE_ID and NYVEXA_UNVERIFIED_ROLE_ID must be different");
        }
        if self.sync_interval_secs < 60 {
            bail!("NYVEXA_SYNC_INTERVAL_SECS must be at least 60 seconds");
        }
        if self.challenge_ttl_secs < 60 {
            bail!("NYVEXA_CHALLENGE_TTL_SECS must be at least 60 seconds");
        }

        let mut managed_roles = HashSet::from([self.member_role_id, self.unverified_role_id]);
        for (rank, role_id) in &self.rank_roles {
            if !managed_roles.insert(*role_id) {
                bail!("Discord role id {role_id} is configured more than once (rank {rank})");
            }
        }
        Ok(())
    }

    pub(crate) fn rank_role_for(&self, rank: &str) -> Option<u64> {
        self.rank_roles
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(rank))
            .map(|(_, role_id)| *role_id)
    }
}

fn required(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing required environment variable {name}"))
}

fn parse_required<T>(name: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    required(name)?
        .parse()
        .with_context(|| format!("invalid value for {name}"))
}

fn parse_optional<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match env::var(name) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("invalid value for {name}")),
        Err(_) => Ok(default),
    }
}
