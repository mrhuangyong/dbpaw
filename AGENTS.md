# AGENTS.md

This file is an **immunity system**. Every rule below exists because an agent
once made this mistake. When you find a new failure mode, add a rule here so
it never happens again.

## Rust / Cargo

- After modifying any `.rs` file, always run `cargo check` before declaring
  done. TypeScript compilation alone does not catch Rust errors.
- When adding a new database driver, do **three** things — not two, not four:
  1. Add the module in `src-tauri/src/db/drivers/<name>.rs`
  2. Register it in the `connect()` match arms in `mod.rs` (line ~384)
  3. Add the `pub mod <name>;` declaration and `use` import at the top of `mod.rs`
- When a driver's type is MySQL-family (mariadb, tidb, starrocks, doris),
  update `is_mysql_family_driver()` in `mod.rs` if the new driver belongs there.
- Oracle tests require the Oracle Instant Client installed locally. The
  `scripts/test-integration.sh` script detects this via `DYLD_LIBRARY_PATH` and
  common paths. Integration tests for Oracle will be **skipped** if the client
  is missing — do not try to "fix" the test to run without it.
- Error messages for connection failures **must** use `conn_failed_error()` in
  `src-tauri/src/db/drivers/mod.rs`. This provides context-aware hints (TLS,
  auth, network). Raw error strings confuse users.
- Integration tests are marked `#[ignore]` and require `IT_DB=<name>` env
  variable to run. They also need Docker. Do not remove `#[ignore]` to "make
  tests pass" in CI.

## Frontend / TypeScript

- `src/services/api.ts` is the **only** file that calls `invoke()`. Never call
  `@tauri-apps/api/core` invoke directly from components or other services.
- When a Tauri command's parameter types change, update **both**:
  - The Rust `#[tauri::command]` signature
  - The corresponding TypeScript wrapper in `src/services/api.ts`
- Mock mode (`VITE_USE_MOCK=true`) is for rapid UI iteration. The mock
  implementations live in `src/services/mocks.ts`. When adding a new API
  method to `api.ts`, always add a corresponding mock entry.
- i18n locale files are TypeScript, not JSON. After adding a new locale file
  in `src/lib/i18n/locales/`, register it in `src/lib/i18n/index.ts`.

## Database Drivers

- Every driver implements the `DatabaseDriver` trait in `mod.rs` (line ~310).
  The trait has required methods (`connect`, `list_databases`, `list_tables`,
  `get_table_structure`, `get_table_metadata`, `get_table_ddl`,
  `get_table_data`, `get_table_data_chunk`, `execute_query`,
  `get_schema_overview`, `close`) and optional ones (`list_routines`,
  `get_routine_ddl`, `execute_query_with_id`).
- Do not add driver-specific connection logic outside the driver module.
  SSH tunneling is handled transparently in the connection layer, not in
  individual drivers.
- SQL statement splitting lives in `mod.rs` (`split_sql_statements`,
  `first_sql_keyword`). Do not reimplement SQL parsing in individual drivers.

## Testing

- Rust integration tests follow three levels:
  - `<db>_integration.rs` — direct driver method testing
  - `<db>_command_integration.rs` — ephemeral connection commands
  - `<db>_stateful_command_integration.rs` — saved connection workflows
  All three must be added to `scripts/test-integration.sh` when adding a new
  database.
- Integration tests run with `--test-threads=1`. Do not change this —
  database containers are shared and parallel tests collide.
- Use `IT_REUSE_LOCAL_DB=1` to skip container creation during local
  development iterations.

## General

- `CLAUDE.md` is a **table of contents** — it points to deeper docs but does
  not duplicate them. If you need architecture details, read `docs/architecture.md`.
  If you need commands, read `docs/commands.md`.
- Do not add instructions to `CLAUDE.md` that apply only in a specific
  context. Add them here in `AGENTS.md` instead, or in a skill file under
  `.claude/skills/`.
- When you encounter a new failure mode during this session, add an entry to
  this file. The file grows as the project's institutional knowledge grows.
