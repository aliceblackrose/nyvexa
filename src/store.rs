use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
use thiserror::Error;

#[derive(Clone, Debug)]
pub(crate) struct NewPendingVerification<'a> {
    pub(crate) discord_user_id: u64,
    pub(crate) character_id: u64,
    pub(crate) character_name: &'a str,
    pub(crate) world: &'a str,
    pub(crate) token_hash: &'a str,
    pub(crate) created_at: i64,
    pub(crate) expires_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingVerification {
    pub(crate) character_id: u64,
    pub(crate) character_name: String,
    pub(crate) world: String,
    pub(crate) token_hash: String,
    pub(crate) expires_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct VerifiedMember {
    pub(crate) discord_user_id: u64,
    pub(crate) character_id: u64,
    pub(crate) character_name: String,
    pub(crate) world: String,
    pub(crate) fc_rank: Option<String>,
    pub(crate) missing_since: Option<i64>,
}

#[derive(Debug, Error)]
pub(crate) enum StoreError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("character is already linked to another Discord account")]
    CharacterAlreadyLinked,
    #[error("stored Discord user id is invalid")]
    InvalidDiscordUserId,
}

#[derive(Clone)]
pub(crate) struct Store {
    pool: SqlitePool,
}

impl Store {
    pub(crate) async fn connect(database_url: &str) -> Result<Self, StoreError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub(crate) async fn put_pending(
        &self,
        verification: NewPendingVerification<'_>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            INSERT INTO pending_verifications (
                discord_user_id, character_id, character_name, world,
                token_hash, expires_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(discord_user_id) DO UPDATE SET
                character_id = excluded.character_id,
                character_name = excluded.character_name,
                world = excluded.world,
                token_hash = excluded.token_hash,
                expires_at = excluded.expires_at,
                created_at = excluded.created_at
            "#,
        )
        .bind(verification.discord_user_id.to_string())
        .bind(verification.character_id as i64)
        .bind(verification.character_name)
        .bind(verification.world)
        .bind(verification.token_hash)
        .bind(verification.expires_at)
        .bind(verification.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn pending(
        &self,
        discord_user_id: u64,
    ) -> Result<Option<PendingVerification>, StoreError> {
        let row = sqlx::query(
            r#"
            SELECT character_id, character_name, world, token_hash, expires_at
            FROM pending_verifications
            WHERE discord_user_id = ?
            "#,
        )
        .bind(discord_user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(PendingVerification {
                character_id: row.try_get::<i64, _>("character_id")? as u64,
                character_name: row.try_get("character_name")?,
                world: row.try_get("world")?,
                token_hash: row.try_get("token_hash")?,
                expires_at: row.try_get("expires_at")?,
            })
        })
        .transpose()
    }

    pub(crate) async fn delete_pending(&self, discord_user_id: u64) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM pending_verifications WHERE discord_user_id = ?")
            .bind(discord_user_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(crate) async fn link_verified(
        &self,
        discord_user_id: u64,
        character_id: u64,
        character_name: &str,
        world: &str,
        fc_rank: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let existing =
            sqlx::query("SELECT discord_user_id FROM verified_members WHERE character_id = ?")
                .bind(character_id as i64)
                .fetch_optional(&mut *tx)
                .await?;

        if let Some(row) = existing {
            let owner = parse_discord_id(row.try_get::<String, _>("discord_user_id")?)?;
            if owner != discord_user_id {
                return Err(StoreError::CharacterAlreadyLinked);
            }
        }

        sqlx::query("DELETE FROM verified_members WHERE discord_user_id = ?")
            .bind(discord_user_id.to_string())
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"
            INSERT INTO verified_members (
                discord_user_id, character_id, character_name, world, fc_rank,
                verified_at, last_fc_seen_at, missing_since
            ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL)
            "#,
        )
        .bind(discord_user_id.to_string())
        .bind(character_id as i64)
        .bind(character_name)
        .bind(world)
        .bind(fc_rank)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub(crate) async fn verified(
        &self,
        discord_user_id: u64,
    ) -> Result<Option<VerifiedMember>, StoreError> {
        let row = sqlx::query(
            r#"
            SELECT discord_user_id, character_id, character_name, world, fc_rank, missing_since
            FROM verified_members
            WHERE discord_user_id = ?
            "#,
        )
        .bind(discord_user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(row_to_verified).transpose()
    }

    pub(crate) async fn all_verified(&self) -> Result<Vec<VerifiedMember>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT discord_user_id, character_id, character_name, world, fc_rank, missing_since
            FROM verified_members
            ORDER BY character_name COLLATE NOCASE
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(row_to_verified).collect()
    }

    pub(crate) async fn mark_present(
        &self,
        discord_user_id: u64,
        rank: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            UPDATE verified_members
            SET fc_rank = ?, last_fc_seen_at = ?, missing_since = NULL
            WHERE discord_user_id = ?
            "#,
        )
        .bind(rank)
        .bind(now)
        .bind(discord_user_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn mark_missing(
        &self,
        discord_user_id: u64,
        now: i64,
    ) -> Result<i64, StoreError> {
        sqlx::query(
            r#"
            UPDATE verified_members
            SET missing_since = COALESCE(missing_since, ?)
            WHERE discord_user_id = ?
            "#,
        )
        .bind(now)
        .bind(discord_user_id.to_string())
        .execute(&self.pool)
        .await?;

        let row =
            sqlx::query("SELECT missing_since FROM verified_members WHERE discord_user_id = ?")
                .bind(discord_user_id.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.try_get("missing_since")?)
    }

    pub(crate) async fn unlink(&self, discord_user_id: u64) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM pending_verifications WHERE discord_user_id = ?")
            .bind(discord_user_id.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM verified_members WHERE discord_user_id = ?")
            .bind(discord_user_id.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn row_to_verified(row: sqlx::sqlite::SqliteRow) -> Result<VerifiedMember, StoreError> {
    Ok(VerifiedMember {
        discord_user_id: parse_discord_id(row.try_get::<String, _>("discord_user_id")?)?,
        character_id: row.try_get::<i64, _>("character_id")? as u64,
        character_name: row.try_get("character_name")?,
        world: row.try_get("world")?,
        fc_rank: row.try_get("fc_rank")?,
        missing_since: row.try_get("missing_since")?,
    })
}

fn parse_discord_id(value: String) -> Result<u64, StoreError> {
    value.parse().map_err(|_| StoreError::InvalidDiscordUserId)
}
