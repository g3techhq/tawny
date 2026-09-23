DEFINE TABLE IF NOT EXISTS subscription TYPE RELATION IN app_user OUT channel SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS created_at ON subscription TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS subscription_user_channel_idx ON subscription FIELDS in, out UNIQUE;
