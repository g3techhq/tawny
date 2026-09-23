DEFINE TABLE IF NOT EXISTS video_progress TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS owner ON video_progress TYPE record<app_user>;
DEFINE FIELD IF NOT EXISTS video_id ON video_progress TYPE string;
DEFINE FIELD IF NOT EXISTS watched ON video_progress TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS progress_seconds ON video_progress TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS audio_only ON video_progress TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS updated_at ON video_progress TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS video_progress_owner_video_idx ON video_progress
  FIELDS owner, video_id UNIQUE;
