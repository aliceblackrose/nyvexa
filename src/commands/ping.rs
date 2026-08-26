use gloamwire::http::CreateApplicationCommand;

pub const NAME: &str = "ping";

pub fn definition() -> CreateApplicationCommand {
    CreateApplicationCommand::chat_input(NAME, "Check whether Nyvexa is online")
}

pub const fn response() -> &'static str {
    "Pong!"
}
