-- A viewer is always signed in as *someone*: first launch mints a guest, and
-- signing up promotes that same record in place so nothing has to be migrated
-- between rows. `email` and `password_hash` stay NONE until then, which is what
-- distinguishes a guest - `is_guest` is the readable form of the same fact.
--
-- Signing in to a *different* account abandons the guest row rather than
-- merging it. Merging two libraries has no correct answer, and offering it
-- would mean asking the viewer to resolve conflicts on their first screen.
DEFINE TABLE IF NOT EXISTS app_user TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS display_name ON app_user TYPE string;
DEFINE FIELD IF NOT EXISTS email ON app_user TYPE option<string>;
DEFINE FIELD IF NOT EXISTS password_hash ON app_user TYPE option<string>;
DEFINE FIELD IF NOT EXISTS is_guest ON app_user TYPE bool DEFAULT true;
DEFINE FIELD IF NOT EXISTS created_at ON app_user TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON app_user TYPE datetime DEFAULT time::now();

-- Guests all carry a NONE email and must not collide over it. Verified against
-- the engine rather than assumed: SurrealDB does not index a NONE, so a plain
-- UNIQUE admits any number of guests and still rejects a duplicate address.
-- (It has no filtered indexes at all - `UNIQUE WHERE` is a parse error.)
DEFINE INDEX IF NOT EXISTS app_user_email_idx ON app_user FIELDS email UNIQUE;
