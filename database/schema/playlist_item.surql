DEFINE TABLE IF NOT EXISTS playlist_item TYPE RELATION IN playlist OUT video SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS position ON playlist_item TYPE int;
DEFINE FIELD IF NOT EXISTS added_at ON playlist_item TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS playlist_item_unique_idx ON playlist_item FIELDS in, out UNIQUE;
