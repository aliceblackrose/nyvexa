# Nyvexa

Nyvexa is a Discord bot built in Rust on top of [Gloamwire](https://github.com/cybellereaper/gloamwire).

## Requirements

- Rust 1.98 or newer.
- A Discord application with a bot token.

## Configuration

Nyvexa reads its Discord bot token from `DISCORD_TOKEN`.

```text
DISCORD_TOKEN=your-bot-token
```

Do not commit a real bot token. `.env` files are ignored by Git, but Nyvexa currently reads the process environment directly rather than loading `.env` automatically.

## Run

Set `DISCORD_TOKEN` in your shell and run:

```sh
cargo run
```

On the first `READY` event, Nyvexa registers its global application commands. Global command updates can take time to become visible in Discord.

## Commands

- `/ping` — verifies that Nyvexa is online and responding to interactions.

## Structure

- `src/main.rs` — process entrypoint.
- `src/config.rs` — environment-backed configuration.
- `src/bot.rs` — Gateway event loop and bot lifecycle.
- `src/commands/` — application-command registration and handlers.

Gloamwire intentionally remains the Discord protocol/transport library; Nyvexa owns bot-specific command and application behavior.
