# Database workflow

`database/schema` is Tawny's desired schema. The server currently embeds it and
runs SurrealKit during startup; `database/backfill.surql` is applied immediately
afterward. Keep schema files and the application checkout at the same revision.

## Scoped operator commands

Copy `.env.surrealkit.template` to an ignored file named for the target
environment, then replace the environment marker, Tailscale service URL, and
credentials:

```powershell
Copy-Item .env.surrealkit.template .env.surrealkit.production
```

Run any SurrealKit command through the scoped launcher:

```powershell
.\scripts\db.ps1 production status
.\scripts\db.ps1 production sync --fail-fast
.\scripts\db.ps1 production seed
.\scripts\db.ps1 production rollout status
.\scripts\db.ps1 production rollout baseline
.\scripts\db.ps1 production rollout plan --name add-tags
.\scripts\db.ps1 production rollout lint <rollout-id>
.\scripts\db.ps1 production rollout start <rollout-id>
.\scripts\db.ps1 production rollout complete <rollout-id>
.\scripts\db.ps1 production rollout rollback <rollout-id>
```

The launcher temporarily applies `.env.surrealkit.<environment>`, runs from the
repository root, and restores the calling process's environment afterward. It
does not modify `.env`, so later development commands keep their local settings.

Tawny does not currently have a `database/seed/` directory, so `seed` has
nothing project-specific to apply until seed files are added. The startup
backfill is not a SurrealKit seed or rollout.

Before switching an existing production database to operator-controlled
rollouts, first move release startup schema changes and `backfill.surql` into
the reviewed rollout workflow. Until then, startup remains authoritative.
