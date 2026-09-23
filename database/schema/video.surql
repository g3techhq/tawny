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
