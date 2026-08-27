# Nyvexa

Nyvexa is a Discord bot built in Rust with [Gloam Commands](https://github.com/aliceblackrose/gloam-macro-commands) on top of [Gloamwire](https://github.com/aliceblackrose/gloamwire).

Gloam Commands owns Nyvexa's slash-command definitions, registration, Gateway runtime, dispatch, command context, and responses.

## Requirements

- Rust 1.98 or newer.
- A Discord application with a bot token.

## Configuration

```text
DISCORD_TOKEN=your-bot-token
```

Do not commit real credentials. `.env` files are ignored by Git.

## Running

Set `DISCORD_TOKEN` and run:

```sh
cargo run
```

Nyvexa builds a `gloam_commands::Framework` with global registration enabled. The framework starts Gloamwire's managed shard set, synchronizes the slash-command registry after Discord's first `READY`, and dispatches command interactions through generated handlers.

## Commands

- `/ping` — verifies that Nyvexa is online and responding to interactions.

## Structure

- `src/main.rs` — application entrypoint and managed Gloam Commands runtime.
- `src/commands/` — slash-command handlers and framework registry.

Nyvexa contains bot-specific application behavior while Gloam Commands and Gloamwire provide the Discord command framework and transport layers.
