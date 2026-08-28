mod check;
mod ping;
mod status;
mod unlink;
mod verify;

use std::sync::Arc;

use anyhow::{Context as _, Result as AppResult};
use gloam_commands::{Context, Framework, Registration, commands};
use gloamwire::model::UserId;
use tracing::error;

use crate::{config::Config, lodestone::LodestoneClient, store::Store};

#[derive(Clone)]
pub(crate) struct CommandData {
    pub(crate) config: Arc<Config>,
    pub(crate) store: Store,
    pub(crate) lodestone: LodestoneClient,
}

impl CommandData {
    pub(crate) fn new(config: Config, store: Store, lodestone: LodestoneClient) -> Self {
        Self {
            config: Arc::new(config),
            store,
            lodestone,
        }
    }
}

pub(crate) fn framework(
    data: CommandData,
    registration: Registration,
) -> gloam_commands::Result<Framework<CommandData>> {
    Framework::builder(data)
        .commands(commands![
            ping::ping,
            verify::verify,
            check::check,
            status::status,
            unlink::unlink,
        ])
        .registration(registration)
        .build()
}

pub(super) fn invoking_user_id(ctx: &Context<CommandData>) -> AppResult<UserId> {
    ctx.interaction()
        .user
        .as_ref()
        .map(|user| user.id)
        .or_else(|| {
            ctx.interaction()
                .member
                .as_ref()
                .and_then(|member| member.user.as_ref())
                .map(|user| user.id)
        })
        .context("interaction did not include the invoking user")
}

pub(super) async fn finish(
    ctx: &Context<CommandData>,
    result: AppResult<String>,
) -> gloam_commands::Result<()> {
    let message = match result {
        Ok(message) => message,
        Err(error) => {
            error!(
                ?error,
                command = ctx.command_path().join(" "),
                "command failed"
            );
            "Something went wrong while processing that request. Please try again later."
                .to_string()
        }
    };

    ctx.reply_ephemeral(message).await
}
