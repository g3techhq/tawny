-- Data and one-time compatibility operations are intentionally separate from
-- SurrealKit's definition-only schema catalog. Each statement is idempotent,
-- so it is safe to run during startup while supporting an upgraded database.

-- A DEFINE FIELD DEFAULT only applies to writes, so rows stored before these
-- fields existed keep NONE and would fail to deserialize into a plain value.
UPDATE channel SET subscription_content = "all" WHERE subscription_content = NONE;
UPDATE video SET audio_only = false WHERE audio_only = NONE;

-- Replace old global identifiers with the owner-scoped indexes the current
-- multi-user model requires. These are no-ops on a fresh database.
REMOVE INDEX IF EXISTS playlist_external_id_idx ON playlist;
REMOVE INDEX IF EXISTS subscription_group_external_id_idx ON subscription_group;

-- A row from before accounts existed was never registered, so it is a guest.
UPDATE app_user SET is_guest = (email = NONE) WHERE is_guest = NONE;
