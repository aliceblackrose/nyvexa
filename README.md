# Nyvexa

Nyvexa is a Discord bot built in Rust on top of [Gloamwire](https://github.com/cybellereaper/gloamwire).

Nyvexa supports two interaction transports:

- Discord Gateway for a traditional long-running bot process.
- Discord HTTP interactions for serverless deployment on Vercel.

## Requirements

- Rust 1.98 or newer.
- A Discord application with a bot token.
- The application's Discord public key when using Vercel.

## Configuration

```text
DISCORD_TOKEN=your-bot-token
DISCORD_PUBLIC_KEY=your-application-public-key
```

`DISCORD_TOKEN` is used by the Gateway process and command registration. `DISCORD_PUBLIC_KEY` is used by the Vercel interaction endpoint to verify every Discord request before processing it.

Do not commit real credentials. `.env` files are ignored by Git.

## Vercel deployment

Vercel's official Rust runtime builds `api/interactions.rs` as the serverless interaction endpoint.

1. Import this repository into Vercel.
2. Add `DISCORD_PUBLIC_KEY` to the Vercel project's environment variables. Copy it from the Discord Developer Portal under the application's General Information page.
3. Deploy the project.
4. In the Discord Developer Portal, set the application's **Interactions Endpoint URL** to:

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

On the first `READY` event, Nyvexa registers its global application commands and handles interactions over the Gateway. This mode is useful for local development and for features that require Gateway events.

Do not run Gateway interaction handling at the same time as the configured HTTP interaction endpoint; Discord uses one interaction-delivery mechanism at a time.

## Commands

- `/ping` — verifies that Nyvexa is online and responding to interactions.

## Structure

- `api/interactions.rs` — Vercel Rust serverless function and Discord signature validation.
- `src/lib.rs` — shared application logic exposed to serverless handlers.
- `src/main.rs` — local Gateway process entrypoint.
- `src/config.rs` — environment-backed Gateway configuration.
- `src/bot.rs` — Gateway event loop and transport adapter.
- `src/commands/` — transport-independent application-command definitions and responses.

Gloamwire remains the Discord protocol/transport library; Nyvexa owns bot-specific command and application behavior.
