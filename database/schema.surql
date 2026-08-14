DEFINE TABLE IF NOT EXISTS channel TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS channel_id ON channel TYPE string;
DEFINE FIELD IF NOT EXISTS name ON channel TYPE string;
DEFINE FIELD IF NOT EXISTS handle ON channel TYPE string;
DEFINE FIELD IF NOT EXISTS avatar_url ON channel TYPE option<string>;
DEFINE FIELD IF NOT EXISTS subscriber_count ON channel TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS subscribed ON channel TYPE bool DEFAULT false;
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
DEFINE INDEX IF NOT EXISTS playlist_external_id_idx ON playlist FIELDS playlist_id UNIQUE;

DEFINE TABLE IF NOT EXISTS playlist_item TYPE RELATION IN playlist OUT video SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS position ON playlist_item TYPE int;
DEFINE FIELD IF NOT EXISTS added_at ON playlist_item TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS playlist_item_unique_idx ON playlist_item FIELDS in, out UNIQUE;

DEFINE TABLE IF NOT EXISTS subscription_group TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS group_id ON subscription_group TYPE string;
DEFINE FIELD IF NOT EXISTS name ON subscription_group TYPE string;
DEFINE FIELD IF NOT EXISTS channel_ids ON subscription_group TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS updated_at ON subscription_group TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS subscription_group_external_id_idx ON subscription_group FIELDS group_id UNIQUE;

DEFINE TABLE IF NOT EXISTS library_state TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS cache_revision ON library_state TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS queue_json ON library_state TYPE string DEFAULT "[]";
DEFINE FIELD IF NOT EXISTS history_json ON library_state TYPE string DEFAULT "[]";
DEFINE FIELD IF NOT EXISTS updated_at ON library_state TYPE datetime DEFAULT time::now();
