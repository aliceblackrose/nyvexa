CREATE TABLE pending_verifications (
    discord_user_id TEXT PRIMARY KEY NOT NULL,
    character_id INTEGER NOT NULL,
    character_name TEXT NOT NULL,
    world TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE verified_members (
    discord_user_id TEXT PRIMARY KEY NOT NULL,
    character_id INTEGER NOT NULL UNIQUE,
    character_name TEXT NOT NULL,
    world TEXT NOT NULL,
    fc_rank TEXT,
    verified_at INTEGER NOT NULL,
    last_fc_seen_at INTEGER,
    missing_since INTEGER
);

CREATE INDEX idx_verified_members_character_id
    ON verified_members(character_id);
