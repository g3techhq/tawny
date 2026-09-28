# Adding a feature

The order that keeps every step compiling: table → server method → server
function → cache invalidation → screen → tests. The `add-feature` skill
(`.claude/skills/add-feature/SKILL.md`) is the checklist form of this page.

The running example: **a note on a playlist**, written by its owner.

## 1. The table

`database/schema/playlist.surql` gains a field, or a new file holds a new
table:

```sql
DEFINE FIELD IF NOT EXISTS note ON playlist TYPE option<string>;
```

`IF NOT EXISTS` everywhere: the files are desired state, applied on every
server start. A data move (renaming, backfilling) goes in
`database/presync.surql` or `backfill.surql`, which run around the sync.

## 2. The server method

The work lives on `AppServerState`, in the `src/server/` file for its area
(`store.rs` for writes to the catalog, `viewer.rs` for an account's own data):

```rust
impl AppServerState {
    pub(crate) async fn set_playlist_note(&self, owner: &str, playlist_id: &str, note: Option<String>) -> Result<()> {
        self.db
            // Playlist ids are the client's and repeat across accounts, so the
            // owner is part of the key.
            .query("UPDATE playlist SET note = $note WHERE playlist_id = $id AND owner = type::record($owner)")
            .bind(("id", playlist_id.to_string()))
            .bind(("note", note))
            .bind(("owner", owner.to_string()))
            .await?;
        Ok(())
    }
}
```

- **The owner comes from the caller** (the server function's `Owner`), and
  the query checks it.
- Values are bound, never formatted into the query.
- Server tests run it against `mem://`: add one next to the others.

## 3. The server function

In `src/api/<area>.rs`:

```rust
#[post(
    "/api/v1/playlists/note",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn set_playlist_note(playlist_id: String, note: Option<String>) -> Result<()> {
    Ok(state.set_playlist_note(&owner.0, &playlist_id, note).await.map_err(server_error)?)
}
```

"Set to", never "toggle": the argument is the state the viewer chose.

## 4. Cache invalidation

`src/data_change.rs` maps each `DataChange` to the cached reads it affects.
A note changes `get_playlist` and, if previews show it, `get_playlist_previews`:
both are already under `DataChange::Playlists`. A new cached read goes into
every list whose changes can affect it, in the same change.

## 5. The screen

- Read through the cached read the screen already uses (`get_playlist`), or a
  new `use_cached(server_fn, (args,))` if it needs its own.
- Write through `AppState`, in the shape of `add_to_playlist`: edit the cached
  reads at once (`update_cached`), then `self.send(DataChange::Playlists,
  api::set_playlist_note(..))`, which calls the server and invalidates.
- The note editor owns its draft in its own component, so a keystroke
  re-renders the box, not the playlist.
- Hooks above any early return; handlers passed down from a busy page are
  `use_callback`s; lists in memos. See [dioxus/patterns.md](dioxus/patterns.md).
- g3-ui components only (see [g3-ui.md](g3-ui.md)).

## 6. Tests and checks

```bash
just check && just test && just lint-strict
```

Then walk it in the browser at 390×844 with the `verify-in-browser` skill:
write a note, see it at once, reload and see it again.
