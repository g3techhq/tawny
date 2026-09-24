//! Database connection-adjacent helpers shared by Tawny's server startup.
//!
//! Schema files live under `database/schema` and are embedded in the server
//! binary. SurrealKit records their hashes in the database, so startup only
//! applies files that changed and can remove schema objects no longer managed
//! by the checked-in schema.

use anyhow::{Context, Result};
use surrealdb::{Surreal, engine::any::Any};

surrealkit::embed_schema!("database/schema");

pub async fn sync_schema(db: &Surreal<Any>) -> Result<()> {
    db.query(include_str!("../database/presync.surql"))
        .await
        .context("apply Tawny pre-sync repairs")?
        .check()
        .context("validate Tawny pre-sync repairs")?;
    embedded_schema::sync(db)
        .await
        .context("sync Tawny schema with SurrealKit")?;
    db.query(include_str!("../database/backfill.surql"))
        .await
        .context("apply Tawny data backfills")?
        .check()
        .context("validate Tawny data backfills")?;
    Ok(())
}
