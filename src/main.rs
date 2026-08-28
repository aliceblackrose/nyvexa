mod clock;
mod commands;
mod config;
mod lodestone;
mod roles;
mod store;
mod verification;

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, anyhow};
use gloam_commands::{DispatchOutcome, Registration};
use gloamwire::{
    RestClient,
    gateway::{GatewayEvent, GatewayIntents, ShardEvent, ShardManager, TypedDispatchEvent},
    model::GuildId,
};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::{commands::CommandData, config::Config, lodestone::LodestoneClient, store::Store};

const INTERACTION_DELAY_WARNING_MILLIS: u64 = 1_500;

#[tokio::main]
async fn main() -> Result<()> {
    install_crypto_provider()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nyvexa=info")),
        )
        .init();

    let config = Config::from_env()?;
    let store = Store::connect(&config.database_url).await?;
    let lodestone = LodestoneClient::new(&config.lodestone_region)?;
    let data = CommandData::new(config, store, lodestone);

    run(data).await
}

fn install_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .map_err(|_| anyhow!("failed to install the Rustls AWS-LC crypto provider"))?;
    }
    Ok(())
}

async fn run(data: CommandData) -> Result<()> {
    let token = data.config.discord_token.clone();
    let rest = RestClient::new(&token).context("failed to create Discord REST client")?;
    let guild_id = GuildId::new(data.config.guild_id);
    let framework = Arc::new(commands::framework(
        data.clone(),
        Registration::Guild(guild_id),
    )?);
    let mut shards = ShardManager::start(
        token,
        GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS,
        &rest,
    )
    .await
    .context("failed to start Discord Gateway shards")?;

    let mut initialized = false;
    let mut sync_task: Option<JoinHandle<()>> = None;

    let result: Result<()> = async {
        loop {
            tokio::select! {
                event = shards.next_event() => {
                    let Some(event) = event else {
                        return Err(anyhow::anyhow!("Discord Gateway shard manager stopped"));
                    };
                    let event = event.context("Discord Gateway shard stopped")?;
                    warn_if_interaction_delayed(&event.event)?;
                    dispatch_command_event(&framework, &rest, &event)?;
                    handle_application_event(
                        &framework,
                        &rest,
                        &data,
                        &event.event,
                        &mut initialized,
                        &mut sync_task,
                    )?;
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.context("failed to listen for shutdown signal")?;
                    info!("shutdown signal received");
                    return Ok(());
                }
            }
        }
    }
    .await;

    if let Some(task) = sync_task {
        task.abort();
    }
    shards
        .shutdown()
        .await
        .context("failed to shut down Discord Gateway shards")?;

    result
}

fn handle_application_event(
    framework: &Arc<gloam_commands::Framework<CommandData>>,
    rest: &RestClient,
    data: &CommandData,
    event: &GatewayEvent,
    initialized: &mut bool,
    sync_task: &mut Option<JoinHandle<()>>,
) -> Result<()> {
    let GatewayEvent::Dispatch(dispatch) = event else {
        return Ok(());
    };

    match dispatch.name.as_str() {
        "READY" => {
            let TypedDispatchEvent::Ready(ready) = dispatch.typed()? else {
                return Ok(());
            };
            info!(user = %ready.user.username, "connected to Discord");

            if !*initialized {
                *initialized = true;

                let framework = Arc::clone(framework);
                let rest = rest.clone();
                let data = data.clone();
                let application_id = ready.application.id;
                *sync_task = Some(tokio::spawn(async move {
                    if let Err(error) = framework
                        .synchronize_commands(&rest, application_id)
                        .await
                        .context("failed to register Nyvexa slash commands")
                    {
                        error!(?error, "failed to initialize Discord commands");
                        return;
                    }

                    roles::run_membership_sync_loop(rest, data).await;
                }));
            }
        }
        "GUILD_MEMBER_ADD" => {
            let TypedDispatchEvent::GuildMemberAdd(event) = dispatch.typed()? else {
                return Ok(());
            };
            let rest = rest.clone();
            let data = data.clone();
            tokio::spawn(async move {
                if let Err(error) = roles::handle_member_add(&rest, &data, event).await {
                    warn!(
                        ?error,
                        "failed to apply verification state to new guild member"
                    );
                }
            });
        }
        _ => {}
    }

    Ok(())
}

fn warn_if_interaction_delayed(event: &GatewayEvent) -> Result<()> {
    let GatewayEvent::Dispatch(dispatch) = event else {
        return Ok(());
    };
    if dispatch.name != "INTERACTION_CREATE" {
        return Ok(());
    }

    let TypedDispatchEvent::InteractionCreate(interaction) = dispatch.typed()? else {
        return Ok(());
    };
    let now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_millis();
    let now_millis = u64::try_from(now_millis).unwrap_or(u64::MAX);
    let age_millis = now_millis.saturating_sub(interaction.id.timestamp_millis());

    if age_millis >= INTERACTION_DELAY_WARNING_MILLIS {
        warn!(
            interaction = %interaction.id,
            age_ms = age_millis,
            "Discord interaction reached Nyvexa late; acknowledgement deadline may be at risk"
        );
    }

    Ok(())
}

fn dispatch_command_event(
    framework: &gloam_commands::Framework<CommandData>,
    rest: &RestClient,
    event: &ShardEvent,
) -> Result<()> {
    match framework.dispatch_shard(rest, event)? {
        DispatchOutcome::Ignored => {}
        DispatchOutcome::Unregistered { name } => {
            debug!(command = %name, "ignored unregistered Discord command");
        }
        DispatchOutcome::AtCapacity { name } => {
            warn!(
                command = name,
                "command rejected because Nyvexa is at capacity"
            );
        }
        DispatchOutcome::Spawned(task) => {
            let command = task.command_name();
            tokio::spawn(async move {
                if let Err(error) = task.join().await {
                    error!(?error, command = command, "command task failed");
                }
            });
        }
    }
    Ok(())
}
