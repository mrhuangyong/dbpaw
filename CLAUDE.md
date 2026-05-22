# DbPaw

Cross-platform database client (Tauri v2, React, Rust). Supports PostgreSQL, MySQL, MariaDB, TiDB, SQLite, SQL Server, ClickHouse, DuckDB.

## Quick Start
- `bun dev:mock` — Frontend-only dev (recommended for UI work)
- `bun tauri dev` — Full Tauri app with Rust backend
- `bun run test:smoke` — Quick validation (typecheck + lint + unit tests)
- `bun run test:all` — Full test suite

## Documentation Index
- **Architecture** → `docs/architecture.md` (frontend/backend structure, patterns, build system, i18n)
- **Commands** → `docs/commands.md` (all dev/test/lint commands)
- **Testing** → `TESTING.md` (3-layer test strategy and coverage)
- **New Database Driver** → `ADD_NEW_DB.md` (step-by-step checklist)
- **Immunity System** → `AGENTS.md` (every line = a past agent failure, prevents repeat mistakes)
- **Design** → `DESIGN.md`

## Critical Rules
- After modifying Rust code, always run `cargo check` (not just TypeScript compilation)
- New database drivers must be registered in `src-tauri/src/db/drivers/mod.rs` enum
- Tauri command parameter changes must sync `src/services/api.ts` type definitions
- Integration tests require Docker; use `IT_REUSE_LOCAL_DB=1` to skip container creation
- All backend calls go through `src/services/api.ts` — never invoke Tauri commands directly

## Core Architecture (tl;dr)
- **Frontend**: `src/` — React/TypeScript, path alias `@/` → `./src/`
- **Backend**: `src-tauri/` — Rust, commands in `commands/`, DB drivers in `db/drivers/`
- **API**: `src/services/api.ts` wraps Tauri `invoke()` with mock mode support
- **Drivers**: All implement `DatabaseDriver` trait; see `db/drivers/mod.rs`
