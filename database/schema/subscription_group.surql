DEFINE TABLE IF NOT EXISTS subscription_group TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS owner ON subscription_group TYPE option<record<app_user>>;
DEFINE FIELD IF NOT EXISTS group_id ON subscription_group TYPE string;
DEFINE FIELD IF NOT EXISTS name ON subscription_group TYPE string;
DEFINE FIELD IF NOT EXISTS channel_ids ON subscription_group TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS updated_at ON subscription_group TYPE datetime DEFAULT time::now();

-- Per owner, not global, for the same reason as playlist ids (playlist.surql).
-- `database/backfill.surql` removes the old global index.
DEFINE INDEX IF NOT EXISTS subscription_group_owner_external_id_idx ON subscription_group
  FIELDS owner, group_id UNIQUE;
