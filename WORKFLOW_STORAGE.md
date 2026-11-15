# v0.5 Stage B: local workflow storage

Stage E2's [saved workflow management](WORKFLOW_MANAGEMENT.md) uses these same
commands for listing, UUID edits and confirmed revision-checked deletion. Its
frontend retention rules now follow the existing unchanged-obsolete-step policy.

Stage E1's [editor](WORKFLOW_EDITOR.md) uses these existing persistence commands.
The save IPC now performs read-only registry prerequisite checks before committing
to reject disappeared app/path selections. Storage still never executes actions;
`Store::save` retains its schema validation, revision checks and transaction boundary.

SQLite is managed by Rust with bundled SQLite (`rusqlite`); no separate database
installation or service is needed. UUID uses the already locked UUID crate.
The database is opened lazily on a worker thread so failures never abort launcher
startup. No visual editor or workflow executor is added.

Location: Tauri `app_local_data_dir()/workflows.sqlite3`, resolved through the
Windows local application data directory and application ID `com.nexus.launcher`.
The default Windows location is
`%LOCALAPPDATA%\com.nexus.launcher\workflows.sqlite3`.
It never depends on the working directory. The directory is created if needed;
existing databases are never replaced. This stage's automated tests use isolated
temporary databases, not the user's production database.

Default dev/debug and installed/release builds share this identifier and data
directory; neither build profile nor checkout path creates isolation. Use a
separate identifier and instance when deliberately building an isolated test app.
Stage G confirmed this from configuration and the path resolver; actual
installed-app restart/upgrade remains pending.

For backup/restore, fully quit every NEXUS instance before copying the database.
Keep any SQLite sidecars with it, or copy the complete data directory while stopped.
Preserve a pre-upgrade copy; never overwrite a live database. Selected paths and
application IDs are plaintext local data. See README.md for the user procedure.

Stage G aligns save-time checks and availability annotations with execution:
sensitive file types are refused before committing through IPC. Older such steps
remain visible for removal/replacement. `Store::save` alone is the schema/revision
layer; native prerequisite and sensitivity checks belong to the IPC boundary.

Database migration version is SQLite `user_version` (currently 1). First migration
creates `workflows` and `workflow_names` atomically. Unknown versions and an
unversioned nonempty database fail explicitly and preserve data. Failed migrations
or corrupt documents never trigger a reset. SQLite transactions use an immediate
write lock, foreign keys, full synchronous durability and a five-second busy timeout.

Each workflow is stored as one generic versioned JSON document, including its
ordered step array; the name/alias index is updated in the same transaction. There
are no per-action database columns. This means a failed step/index update cannot
partially overwrite a workflow. Deletion cascades only that workflow's names.

Payload fields: `id` (non-nil UUID), `name`, nullable `description`, `aliases`,
`steps`, `format_version` (1). Each step contains a non-nil UUID `id`, `action_id`,
`action_schema_version`, JSON `parameters`, contiguous zero-based `order`, and
nullable `label`. The caller preserves IDs across edits, renames and reordering.
Rust owns `created_at`/`updated_at` (UTC Unix milliseconds) and revision (starts at 1).

Tauri commands:

- `list_workflows()` and `get_workflow(id)` return stored workflows with
  `executable` and `issues` availability annotations.
- `save_workflow(payload, expectedRevision)` creates with null expected revision;
  editing requires the exact current revision. Stale saves return `revision_conflict`.
- `delete_workflow(id, expectedRevision, confirmed)` requires explicit confirmation
  and the current revision. A future UI must show the exact workflow before asking.

Names are trimmed, 1–120 characters, and share a case-insensitive uniqueness
namespace with aliases. Up to 20 aliases, 64 steps, 4096 description bytes,
240 label bytes, and 64 KiB of parameters per step are accepted. Step order and IDs
must be unique. New steps use Stage A validation and registered versions. File
paths must satisfy the existing safe path syntax. Lock and sleep must be last.
No request executes an action or interprets shell/script text.

Missing application/file targets are retained and annotated on reads; application
availability uses the current in-memory catalog (an unloaded catalog is reported
as unavailable until loaded). Availability is a snapshot and execution will need
fresh validation in a later stage. File checks may wait for Windows network paths.

Existing missing/obsolete action references remain stored. They may be retained
unchanged while renaming/reordering or repairing other steps; a replacement action
and parameters must pass current registry validation. Unknown actions cannot be
introduced into new workflows. Unavailable workflows are not marked executable.
This stage does not migrate obsolete action parameters automatically.

## Verification

- Targeted SQLite tests: 5 passed. These cover CRUD, stable IDs, rename, ordering,
  reopen persistence, duplicate names/aliases, stale revisions, confirmation for
  deletion, invalid parameters, terminal positioning, obsolete action retention and
  repair, fresh migration, rejected future/unversioned databases, and transaction
  rollback using an injected SQLite trigger failure.
- Full Rust suite: 89 passed, zero failed, 2 existing opt-in/fixture tests ignored.
- Frontend suite: 68 passed, zero failed.
- TypeScript/Vite production build and Windows native debug build passed using
  locked, offline dependencies after the initial SQLite dependency download.
- Formatting and diff checks passed. Cargo emitted the existing sandbox profile
  path warning; it did not prevent compilation or tests.

Stage B built no editor, workflow execution engine or new installer. Production-data
CRUD through the desktop IPC and a fresh manual legacy-launcher regression were
not performed; storage tests exercised real SQLite databases in temporary folders.

Stage C now loads an owned workflow snapshot using `Store::get`, drops the database
connection, and validates it before execution. Edits/deletion after loading do not
change that run. See [WORKFLOW_EXECUTION.md](WORKFLOW_EXECUTION.md).
