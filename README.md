# Nyvexa

Nyvexa is a Discord bot built in Rust with [Gloam Commands](https://github.com/aliceblackrose/gloam-macro-commands) on top of [Gloamwire](https://github.com/aliceblackrose/gloamwire).

Nyvexa supports two interaction transports:

- Discord Gateway for a traditional long-running bot process.
- Discord HTTP interactions for serverless deployment on Vercel.

Gloam Commands owns Nyvexa's slash-command definitions, registry, Discord registration metadata, Gateway dispatch, command context, and responses. The Vercel endpoint remains a thin signed HTTP transport adapter because Gloam Commands currently exposes Gateway-event dispatch rather than a raw HTTP-interaction dispatcher.

## Requirements

- Rust 1.98 or newer.
- A Discord application with a bot token.
- The application's Discord public key when using Vercel.

## Configuration

```text
DISCORD_TOKEN=your-bot-token
DISCORD_APPLICATION_ID=your-application-id
DISCORD_PUBLIC_KEY=your-application-public-key
```

`DISCORD_TOKEN` is used by the Gateway process and command registration. `DISCORD_APPLICATION_ID` is used by the one-shot command registrar. `DISCORD_PUBLIC_KEY` is used by the Vercel interaction endpoint to verify every Discord request before processing it.

Do not commit real credentials. `.env` files are ignored by Git.

## Vercel deployment

Vercel's official Rust runtime builds `api/interactions.rs` as the serverless interaction endpoint.

1. Register Nyvexa's global commands once by setting `DISCORD_TOKEN` and `DISCORD_APPLICATION_ID`, then running:

```sh
cargo run --bin register
```

2. Import this repository into Vercel.
3. Add `DISCORD_PUBLIC_KEY` to the Vercel project's environment variables. Copy it from the Discord Developer Portal under the application's General Information page.
4. Deploy the project.
5. In the Discord Developer Portal, set the application's **Interactions Endpoint URL** to:

```text
https://YOUR-VERCEL-DOMAIN/api/interactions
```

Discord will send a signed `PING` request when the endpoint is saved. Nyvexa validates `X-Signature-Ed25519` and `X-Signature-Timestamp` before returning the required `PONG` response.

Once an Interactions Endpoint URL is configured, slash-command interactions are delivered over HTTP instead of `INTERACTION_CREATE` on the Gateway. The Vercel deployment therefore does not require a persistent Gateway WebSocket.

## Local Gateway mode

Set `DISCORD_TOKEN` and run:

```sh
cargo run
```

Nyvexa builds a `gloam_commands::Framework` with global registration enabled. The framework starts Gloamwire's managed shard set, synchronizes the slash-command registry once after Discord's first `READY`, and dispatches command interactions through the generated handlers.

Do not run Gateway interaction handling at the same time as the configured HTTP interaction endpoint; Discord uses one interaction-delivery mechanism at a time.

## Commands

- `/ping` — verifies that Nyvexa is online and responding to interactions.

## Structure

- `api/interactions.rs` — Vercel Rust serverless function and Discord signature validation.
- `src/bin/register.rs` — one-shot global slash-command synchronization through Gloam Commands.
- `src/lib.rs` — shared application logic exposed to serverless handlers.
- `src/main.rs` — local managed Gloam Commands Gateway runtime.
- `src/config.rs` — environment-backed Gateway configuration.
- `src/commands/` — Gloam Commands handlers plus the Vercel HTTP response adapter.

Gloam Commands owns slash-command framework behavior; Gloamwire remains the underlying Discord protocol and transport library; Nyvexa owns bot-specific application behavior.
