use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

pub(crate) fn unix_timestamp() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX epoch")?
        .as_secs() as i64)
}
