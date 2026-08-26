use gloamwire::{
    RestClient,
    gateway::{GatewayConfig, GatewayConnection, GatewayEvent, GatewayIntents, TypedDispatchEvent},
    http::CreateInteractionResponseQuery,
};

use crate::{commands, config::Config};

pub struct Bot {
    rest: RestClient,
    gateway: GatewayConnection,
    commands_registered: bool,
}

impl Bot {
    pub async fn connect(config: Config) -> commands::CommandResult<Self> {
        let rest = RestClient::new(config.discord_token.clone())?;
        let gateway = GatewayConnection::connect(GatewayConfig::new(
            config.discord_token,
            GatewayIntents::GUILDS,
        ))
        .await?;

        Ok(Self {
            rest,
            gateway,
            commands_registered: false,
        })
    }

    pub async fn run(&mut self) -> commands::CommandResult<()> {
        loop {
            let event = self.gateway.next_event().await?;
            let GatewayEvent::Dispatch(dispatch) = event else {
                continue;
            };

            match dispatch.typed()? {
                TypedDispatchEvent::Ready(ready) => {
                    println!("connected as {} ({})", ready.user.username, ready.user.id);

                    if !self.commands_registered {
                        commands::register_global(&self.rest, ready.application.id).await?;
                        self.commands_registered = true;
                        println!("registered global application commands");
                    }
                }
                TypedDispatchEvent::InteractionCreate(interaction) => {
                    let response = match commands::response(&interaction) {
                        Ok(response) => response,
                        Err(error) => {
                            eprintln!("failed to build interaction response: {error}");
                            continue;
                        }
                    };

                    let Some(response) = response else {
                        continue;
                    };

                    if let Err(error) = self
                        .rest
                        .create_interaction_response(
                            interaction.id,
                            &interaction.token,
                            &response,
                            &CreateInteractionResponseQuery::default(),
                        )
                        .await
                    {
                        eprintln!("failed to send interaction response: {error}");
                    }
                }
                _ => {}
            }
        }
    }
}
