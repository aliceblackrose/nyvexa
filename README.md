# Nyvexa

Nyvexa is a Rust Discord bot built with [Gloam Commands](https://github.com/aliceblackrose/gloam-macro-commands) on top of [Gloamwire](https://github.com/aliceblackrose/gloamwire). It verifies Final Fantasy XIV Free Company members directly against Square Enix's public Lodestone pages and manages Discord access from that verification state.

Nyvexa does not use XIVAPI, Nodestone, or another third-party FFXIV data API.

## Verification flow

1. A new Discord member receives the configured `Unverified` role.
2. They run `/verify character:<name> world:<world>`.
3. Nyvexa resolves the exact character on Lodestone and generates a cryptographically random, single-use challenge.
4. The user places the challenge in the character's public Lodestone self-introduction.
5. They run `/check`.
6. Nyvexa verifies the challenge against the exact Lodestone character ID, confirms the profile points to the configured Free Company, and independently confirms that character ID appears in the FC roster.
7. Nyvexa stores the verified Discord-to-character link, removes `Unverified`, adds the FC member role, and optionally maps the in-game FC rank to a Discord role.
8. A background reconciliation job periodically re-reads the FC roster. Rank changes update Discord roles. Missing members enter a grace period before FC access is revoked.

Challenge tokens are never persisted in plaintext. Nyvexa stores only their SHA-256 digests. Lodestone character IDs are unique in the verification database, preventing two Discord users from claiming the same character.

## Commands

- `/ping` — check whether Nyvexa is online.
- `/verify character:<name> world:<world>` — start or replace a pending Lodestone ownership challenge.
- `/check` — validate the pending challenge and current FC membership.
- `/status` — show the linked character and last known FC rank.
- `/unlink` — remove the character link and revoke FC roles.

## Discord setup

Create a Discord application and bot, then:

- enable the **Server Members Intent** in the Discord Developer Portal;
- invite the bot with the `bot` and `applications.commands` scopes;
- grant it `Manage Roles`, plus normal read/send permissions in the verification channel;
- place the bot's highest role above `Unverified`, the FC member role, and every configured FC-rank role;
- deny normal server channels to `@everyone`/`Unverified` and allow them to the configured FC member role.

Nyvexa uses guild command registration so command changes propagate quickly.

## Configuration

Copy `.env.example` into your deployment environment. Nyvexa reads environment variables directly and does not parse `.env` files itself.

Required values:

| Variable | Meaning |
| --- | --- |
| `DISCORD_TOKEN` | Discord bot token |
| `NYVEXA_GUILD_ID` | Discord server ID |
| `NYVEXA_FC_ID` | Lodestone Free Company ID from `/lodestone/freecompany/<id>/` |
| `NYVEXA_MEMBER_ROLE_ID` | Role granted to confirmed FC members |
| `NYVEXA_UNVERIFIED_ROLE_ID` | Restricted role used before verification or after membership expires |

Optional values:

| Variable | Default | Meaning |
| --- | ---: | --- |
| `NYVEXA_LODESTONE_REGION` | `na` | Lodestone host prefix: `na`, `eu`, `jp`, `fr`, or `de` |
| `NYVEXA_RANK_ROLES` | `{}` | JSON object mapping exact FC rank names to Discord role IDs |
| `NYVEXA_DATABASE_URL` | `sqlite://nyvexa.db?mode=rwc` | SQLx SQLite connection URL |
| `NYVEXA_SYNC_INTERVAL_SECS` | `900` | Full FC roster reconciliation interval |
| `NYVEXA_MEMBERSHIP_GRACE_SECS` | `43200` | Time a character may be missing before roles are revoked |
| `NYVEXA_CHALLENGE_TTL_SECS` | `1800` | Ownership challenge lifetime |

Example rank mapping:

```text
NYVEXA_RANK_ROLES={"Master":"123456789012345678","Officer":"234567890123456789","Member":"345678901234567890"}
```

## Running

```bash
cargo run --release
```

The default database is a local SQLite file. Migrations run automatically at startup.

## Development

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Architecture

- `src/main.rs` owns the Gloamwire Gateway because Nyvexa needs the privileged guild-member event stream in addition to slash-command interactions.
- `src/commands/` contains Gloam Commands slash handlers.
- `src/lodestone.rs` isolates Lodestone HTTP and HTML parsing.
- `src/store.rs` owns SQLite persistence and migrations.
- `src/roles.rs` owns Discord role reconciliation and join-time access enforcement.
- `src/verification.rs` owns challenge generation, hashing, and biography matching.

A failed Lodestone request does not revoke roles. A successfully fetched roster must continue to omit a character for the configured grace period before Nyvexa removes FC access.

Nyvexa never asks users for Square Enix credentials, Mog Station credentials, one-time passwords, session cookies, or Discord passwords.
