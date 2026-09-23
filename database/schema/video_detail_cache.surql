DEFINE TABLE IF NOT EXISTS video_detail_cache TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS video_id ON video_detail_cache TYPE string;
DEFINE FIELD IF NOT EXISTS payload_json ON video_detail_cache TYPE string;
DEFINE FIELD IF NOT EXISTS fetched_at ON video_detail_cache TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS video_detail_cache_external_id_idx ON video_detail_cache FIELDS video_id UNIQUE;
