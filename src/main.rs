mod clock;
mod commands;
mod config;
mod lodestone;
mod roles;
mod store;
mod verification;

use anyhow::{Context as _, Result};
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

#[tokio::main]
async fn main() -> Result<()> {
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

async fn run(data: CommandData) -> Result<()> {
    let token = data.config.discord_token.clone();
    let rest = RestClient::new(&token).context("failed to create Discord REST client")?;
    let guild_id = GuildId::new(data.config.guild_id);
    let framework = commands::framework(data.clone(), Registration::Guild(guild_id))?;
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
                    handle_application_event(
                        &framework,
                        &rest,
                        &data,
                        &event.event,
                        &mut initialized,
                        &mut sync_task,
                    )
                    .await?;
                    dispatch_command_event(&framework, &rest, &event)?;
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

async fn handle_application_event(
    framework: &gloam_commands::Framework<CommandData>,
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
                framework
                    .synchronize_commands(rest, ready.application.id)
                    .await
                    .context("failed to register Nyvexa slash commands")?;

                let rest = rest.clone();
                let data = data.clone();
                *sync_task = Some(tokio::spawn(async move {
                    roles::run_membership_sync_loop(rest, data).await;
                }));
                *initialized = true;
            }
        }
        "GUILD_MEMBER_ADD" => {
            let TypedDispatchEvent::GuildMemberAdd(event) = dispatch.typed()? else {
                return Ok(());
            };
            if let Err(error) = roles::handle_member_add(rest, data, event).await {
                warn!(
                    ?error,
                    "failed to apply verification state to new guild member"
                );
            }
        }
        _ => {}
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
