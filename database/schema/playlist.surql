DEFINE TABLE IF NOT EXISTS playlist TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS owner ON playlist TYPE option<record<app_user>>;
DEFINE FIELD IF NOT EXISTS playlist_id ON playlist TYPE string;
DEFINE FIELD IF NOT EXISTS name ON playlist TYPE string;
DEFINE FIELD IF NOT EXISTS description ON playlist TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS pinned ON playlist TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS video_ids ON playlist TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS updated_at ON playlist TYPE datetime DEFAULT time::now();

-- Playlist ids are chosen by the client and some are fixed - every library
-- starts with a "watch-later". Two accounts must be able to hold the same id,
-- so uniqueness is per owner, not global. `database/backfill.surql` runs after
-- the schema sync and removes the old global index from an upgraded database.
DEFINE INDEX IF NOT EXISTS playlist_owner_external_id_idx ON playlist
  FIELDS owner, playlist_id UNIQUE;
