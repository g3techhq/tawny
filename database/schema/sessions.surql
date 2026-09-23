-- axum_session's store (src/auth/session_store.rs). It must exist up front:
-- SurrealDB 3.x errors when selecting from an undefined table, and session ID
-- generation selects before the first upsert could ever create it.
DEFINE TABLE IF NOT EXISTS sessions TYPE NORMAL SCHEMALESS PERMISSIONS NONE;

DEFINE INDEX IF NOT EXISTS sessions_sessionid_idx ON sessions FIELDS sessionid UNIQUE;
