DEFINE TABLE IF NOT EXISTS library_state TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS owner ON library_state TYPE option<record<app_user>>;
DEFINE FIELD IF NOT EXISTS cache_revision ON library_state TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS queue_json ON library_state TYPE string DEFAULT "[]";
DEFINE FIELD IF NOT EXISTS history_json ON library_state TYPE string DEFAULT "[]";
DEFINE FIELD IF NOT EXISTS updated_at ON library_state TYPE datetime DEFAULT time::now();

-- Same NONE handling as the email index in app_user.surql: the pre-accounts
-- singleton row carries no owner and does not collide with the per-user rows
-- beside it.
DEFINE INDEX IF NOT EXISTS library_state_owner_idx ON library_state FIELDS owner UNIQUE;
