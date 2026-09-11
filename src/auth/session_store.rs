use std::sync::Arc;

use async_trait::async_trait;
use axum_session::{DatabaseError, DatabasePool};
use surrealdb::{Connection, Surreal};
use surrealdb_types::{SurrealValue, Table, Value};

#[derive(SurrealValue)]
struct StoredSession {
    sessionstore: String,
    sessionexpires: String,
    sessionid: String,
}

/// SurrealDB v3 adapter shared with Greenside Partee and Media Mancer.
#[derive(Clone, Debug)]
pub struct SurrealSessionPool<C>
where
    C: Connection,
{
    connection: Arc<Surreal<C>>,
}

impl<C> SurrealSessionPool<C>
where
    C: Connection,
{
    pub fn new(connection: Arc<Surreal<C>>) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl<C> DatabasePool for SurrealSessionPool<C>
where
    C: Connection,
{
    async fn initiate(&self, _table_name: &str) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn count(&self, table_name: &str) -> Result<i64, DatabaseError> {
        let mut response = self
            .connection
            .query("SELECT VALUE count() FROM $table_name GROUP ALL")
            .bind(("table_name", Table::from(table_name)))
            .await
            .map_err(|error| DatabaseError::GenericSelectError(error.to_string()))?;
        Ok(response
            .take::<Option<i64>>(0)
            .map_err(|error| DatabaseError::GenericNotSupportedError(error.to_string()))?
            .unwrap_or_default())
    }

    async fn store(
        &self,
        id: &str,
        session: &str,
        expires: i64,
        table_name: &str,
    ) -> Result<(), DatabaseError> {
        let _: Option<Value> = self
            .connection
            .upsert((table_name, id))
            .content(StoredSession {
                sessionstore: session.to_string(),
                sessionexpires: expires.to_string(),
                sessionid: id.to_string(),
            })
            .await
            .map_err(|error| DatabaseError::GenericInsertError(error.to_string()))?;
        Ok(())
    }

    async fn load(&self, id: &str, table_name: &str) -> Result<Option<String>, DatabaseError> {
        let mut response = self
            .connection
            .query("SELECT VALUE sessionstore FROM $table_name WHERE sessionid = $session_id AND (sessionexpires = NONE OR type::number(sessionexpires) > $expires) LIMIT 1")
            .bind(("table_name", Table::from(table_name)))
            .bind(("session_id", id.to_string()))
            .bind(("expires", now()))
            .await
            .map_err(|error| DatabaseError::GenericSelectError(error.to_string()))?;
        response
            .take::<Option<String>>(0)
            .map_err(|error| DatabaseError::GenericNotSupportedError(error.to_string()))
    }

    async fn delete_one_by_id(&self, id: &str, table_name: &str) -> Result<(), DatabaseError> {
        self.connection
            .query("DELETE $table_name WHERE sessionid = $session_id")
            .bind(("table_name", Table::from(table_name)))
            .bind(("session_id", id.to_string()))
            .await
            .map_err(|error| DatabaseError::GenericDeleteError(error.to_string()))?;
        Ok(())
    }

    async fn exists(&self, id: &str, table_name: &str) -> Result<bool, DatabaseError> {
        Ok(self.load(id, table_name).await?.is_some())
    }

    async fn delete_by_expiry(&self, table_name: &str) -> Result<Vec<String>, DatabaseError> {
        let mut response = self
            .connection
            .query("DELETE $table_name WHERE sessionexpires != NONE AND type::number(sessionexpires) < $expires RETURN BEFORE")
            .bind(("table_name", Table::from(table_name)))
            .bind(("expires", now()))
            .await
            .map_err(|error| DatabaseError::GenericDeleteError(error.to_string()))?;
        response
            .take::<Vec<String>>("sessionid")
            .map_err(|error| DatabaseError::GenericSelectError(error.to_string()))
    }

    async fn delete_all(&self, table_name: &str) -> Result<(), DatabaseError> {
        self.connection
            .query("DELETE $table_name")
            .bind(("table_name", Table::from(table_name)))
            .await
            .map_err(|error| DatabaseError::GenericDeleteError(error.to_string()))?;
        Ok(())
    }

    async fn get_ids(&self, table_name: &str) -> Result<Vec<String>, DatabaseError> {
        let mut response = self
            .connection
            .query("SELECT VALUE sessionid FROM $table_name WHERE sessionexpires = NONE OR type::number(sessionexpires) > $expires")
            .bind(("table_name", Table::from(table_name)))
            .bind(("expires", now()))
            .await
            .map_err(|error| DatabaseError::GenericSelectError(error.to_string()))?;
        response
            .take::<Vec<String>>(0)
            .map_err(|error| DatabaseError::GenericSelectError(error.to_string()))
    }

    fn auto_handles_expiry(&self) -> bool {
        false
    }
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
