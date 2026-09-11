DEFINE TABLE IF NOT EXISTS channel TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS channel_id ON channel TYPE string;
DEFINE FIELD IF NOT EXISTS name ON channel TYPE string;
DEFINE FIELD IF NOT EXISTS handle ON channel TYPE string;
DEFINE FIELD IF NOT EXISTS avatar_url ON channel TYPE option<string>;
DEFINE FIELD IF NOT EXISTS subscriber_count ON channel TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS subscribed ON channel TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS subscription_content ON channel TYPE string DEFAULT "all";
DEFINE FIELD IF NOT EXISTS description ON channel TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS banner_url ON channel TYPE option<string>;
DEFINE FIELD IF NOT EXISTS last_polled_at ON channel TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS last_full_refresh_at ON channel TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS last_websub_at ON channel TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS websub_status ON channel TYPE option<string>;
DEFINE FIELD IF NOT EXISTS websub_requested_at ON channel TYPE option<datetime>;
DEFINE INDEX IF NOT EXISTS channel_external_id_idx ON channel FIELDS channel_id UNIQUE;

DEFINE TABLE IF NOT EXISTS video TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS video_id ON video TYPE string;
DEFINE FIELD IF NOT EXISTS title ON video TYPE string;
DEFINE FIELD IF NOT EXISTS channel_id ON video TYPE string;
DEFINE FIELD IF NOT EXISTS channel_name ON video TYPE string;
DEFINE FIELD IF NOT EXISTS thumbnail_url ON video TYPE string;
DEFINE FIELD IF NOT EXISTS published_at ON video TYPE string;
DEFINE FIELD IF NOT EXISTS published_sort ON video TYPE string;
DEFINE FIELD IF NOT EXISTS duration_seconds ON video TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS view_count ON video TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS is_live ON video TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS is_short ON video TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS progress_seconds ON video TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS watched ON video TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS audio_only ON video TYPE bool DEFAULT false;
DEFINE INDEX IF NOT EXISTS video_external_id_idx ON video FIELDS video_id UNIQUE;
DEFINE INDEX IF NOT EXISTS video_channel_published_idx ON video FIELDS channel_id, published_sort;

DEFINE TABLE IF NOT EXISTS video_detail_cache TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS video_id ON video_detail_cache TYPE string;
DEFINE FIELD IF NOT EXISTS payload_json ON video_detail_cache TYPE string;
DEFINE FIELD IF NOT EXISTS fetched_at ON video_detail_cache TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS video_detail_cache_external_id_idx ON video_detail_cache FIELDS video_id UNIQUE;

DEFINE TABLE IF NOT EXISTS app_user TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS display_name ON app_user TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON app_user TYPE datetime DEFAULT time::now();

DEFINE TABLE IF NOT EXISTS subscription TYPE RELATION IN app_user OUT channel SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS created_at ON subscription TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS subscription_user_channel_idx ON subscription FIELDS in, out UNIQUE;

DEFINE TABLE IF NOT EXISTS playlist TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS owner ON playlist TYPE option<record<app_user>>;
DEFINE FIELD IF NOT EXISTS playlist_id ON playlist TYPE string;
DEFINE FIELD IF NOT EXISTS name ON playlist TYPE string;
DEFINE FIELD IF NOT EXISTS description ON playlist TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS pinned ON playlist TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS video_ids ON playlist TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS updated_at ON playlist TYPE datetime DEFAULT time::now();

DEFINE TABLE IF NOT EXISTS playlist_item TYPE RELATION IN playlist OUT video SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS position ON playlist_item TYPE int;
DEFINE FIELD IF NOT EXISTS added_at ON playlist_item TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS playlist_item_unique_idx ON playlist_item FIELDS in, out UNIQUE;

DEFINE TABLE IF NOT EXISTS subscription_group TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS group_id ON subscription_group TYPE string;
DEFINE FIELD IF NOT EXISTS name ON subscription_group TYPE string;
DEFINE FIELD IF NOT EXISTS channel_ids ON subscription_group TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS updated_at ON subscription_group TYPE datetime DEFAULT time::now();

DEFINE TABLE IF NOT EXISTS library_state TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS cache_revision ON library_state TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS queue_json ON library_state TYPE string DEFAULT "[]";
DEFINE FIELD IF NOT EXISTS history_json ON library_state TYPE string DEFAULT "[]";
DEFINE FIELD IF NOT EXISTS updated_at ON library_state TYPE datetime DEFAULT time::now();

-- Backfill for channels stored before subscription_content existed. A DEFINE
-- FIELD DEFAULT only applies to writes, so rows written earlier keep NONE and
-- would fail to deserialize into a plain string. Idempotent: once every row has
-- a value this matches nothing.

-- --------------------------------------------------------------------------
-- Accounts
--
-- A viewer is always signed in as *someone*: first launch mints a guest, and
-- signing up promotes that same record in place so nothing has to be migrated
-- between rows. `email` and `password_hash` stay NONE until then, which is what
-- distinguishes a guest - `is_guest` is the readable form of the same fact.
--
-- Signing in to a *different* account abandons the guest row rather than
-- merging it. Merging two libraries has no correct answer, and offering it
-- would mean asking the viewer to resolve conflicts on their first screen.
-- --------------------------------------------------------------------------
DEFINE FIELD IF NOT EXISTS email ON app_user TYPE option<string>;
DEFINE FIELD IF NOT EXISTS password_hash ON app_user TYPE option<string>;
DEFINE FIELD IF NOT EXISTS is_guest ON app_user TYPE bool DEFAULT true;
DEFINE FIELD IF NOT EXISTS updated_at ON app_user TYPE datetime DEFAULT time::now();
-- Guests all carry a NONE email and must not collide over it. Verified against
-- the engine rather than assumed: SurrealDB does not index a NONE, so a plain
-- UNIQUE admits any number of guests and still rejects a duplicate address.
-- (It has no filtered indexes at all - `UNIQUE WHERE` is a parse error.)
DEFINE INDEX IF NOT EXISTS app_user_email_idx ON app_user FIELDS email UNIQUE;

-- Only the hash is stored. A stolen database then yields no usable bearer
-- token, the same reason the password beside it is hashed.
DEFINE TABLE IF NOT EXISTS user_session TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS token_hash ON user_session TYPE string;
DEFINE FIELD IF NOT EXISTS user ON user_session TYPE record<app_user>;
DEFINE FIELD IF NOT EXISTS created_at ON user_session TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS last_seen_at ON user_session TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS expires_at ON user_session TYPE datetime;
DEFINE INDEX IF NOT EXISTS user_session_token_idx ON user_session FIELDS token_hash UNIQUE;
DEFINE INDEX IF NOT EXISTS user_session_user_idx ON user_session FIELDS user;

-- --------------------------------------------------------------------------
-- Per-user library state
--
-- `channel` and `video` stay shared. They are YouTube's facts, not the
-- viewer's: two accounts on one server should not fetch and store the same
-- upload twice. Everything that is genuinely a *choice* - what you follow, what
-- you saved, where you stopped watching - hangs off the account instead.
-- --------------------------------------------------------------------------
-- Who follows what. Presence of a row is not enough on its own: an explicit
-- `subscribed` flag lets an unsubscribe be recorded and synced rather than
-- inferred from a missing row, which is what the client already sends.
--
-- `channel.subscribed` survives beside this as a derived flag meaning "someone
-- on this instance follows it". That is precisely the question the poller and
-- the WebSub renewals ask, so they keep working untouched - and it must be
-- recomputed from this table whenever a subscription changes, never written
-- directly. See `refresh_channel_interest`.
DEFINE TABLE IF NOT EXISTS user_subscription TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS owner ON user_subscription TYPE record<app_user>;
DEFINE FIELD IF NOT EXISTS channel_id ON user_subscription TYPE string;
DEFINE FIELD IF NOT EXISTS subscribed ON user_subscription TYPE bool DEFAULT true;
DEFINE FIELD IF NOT EXISTS content ON user_subscription TYPE string DEFAULT "all";
DEFINE FIELD IF NOT EXISTS updated_at ON user_subscription TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS user_subscription_owner_channel_idx ON user_subscription
  FIELDS owner, channel_id UNIQUE;
DEFINE INDEX IF NOT EXISTS user_subscription_channel_idx ON user_subscription FIELDS channel_id;

DEFINE FIELD IF NOT EXISTS owner ON subscription_group TYPE option<record<app_user>>;

-- Playlist and group ids are chosen by the client and some are fixed - every
-- library starts with a "watch-later". Two accounts must be able to hold the
-- same id, so uniqueness is per owner, not global.
--
-- The old global indexes are removed by `database/backfill.surql`, before
-- these owner-scoped replacements are created for an existing deployment.
DEFINE INDEX IF NOT EXISTS playlist_owner_external_id_idx ON playlist
  FIELDS owner, playlist_id UNIQUE;
DEFINE INDEX IF NOT EXISTS subscription_group_owner_external_id_idx ON subscription_group
  FIELDS owner, group_id UNIQUE;

DEFINE TABLE IF NOT EXISTS video_progress TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS owner ON video_progress TYPE record<app_user>;
DEFINE FIELD IF NOT EXISTS video_id ON video_progress TYPE string;
DEFINE FIELD IF NOT EXISTS watched ON video_progress TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS progress_seconds ON video_progress TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS audio_only ON video_progress TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS updated_at ON video_progress TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS video_progress_owner_video_idx ON video_progress
  FIELDS owner, video_id UNIQUE;

DEFINE FIELD IF NOT EXISTS owner ON library_state TYPE option<record<app_user>>;
-- Same NONE handling as the email index above: the pre-accounts singleton row
-- carries no owner and does not collide with the per-user rows beside it.
DEFINE INDEX IF NOT EXISTS library_state_owner_idx ON library_state FIELDS owner UNIQUE;
