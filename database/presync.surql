-- Data repairs that must land before SurrealKit syncs the schema. SurrealKit
-- rewrites every DEFINE to OVERWRITE, so a changed schema file rebuilds its
-- indexes, and a UNIQUE index rebuild fails the whole startup on any row that
-- slipped past it. Each statement is idempotent and a no-op on a fresh
-- database, where the tables do not exist yet.

-- Some development databases hold two rows per (owner, channel_id) that the
-- unique index never saw. Keep the most recently written answer. WITH NOINDEX
-- because the stale index hides the duplicates from indexed lookups.
IF (INFO FOR DB).tables.user_subscription != NONE {
    FOR $group IN (
        SELECT owner, channel_id, array::sort::desc(array::group([updated_at, id])) AS rows
        FROM user_subscription WITH NOINDEX
        GROUP BY owner, channel_id
    ) {
        IF array::len($group.rows) > 1 {
            DELETE array::slice($group.rows, 1).map(|$row| $row[1]);
        };
    };
};
