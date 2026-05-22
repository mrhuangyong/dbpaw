# Architecture

DbPaw separates frontend (React/TypeScript) from backend (Rust) with communication via Tauri commands.

## Frontend (React + TypeScript)

**Directory Structure:**
- `src/components/ui/` - Shadcn/UI components (base UI primitives)
- `src/components/business/` - Business logic components:
  - `Editor/` - SQL editor (Monaco/CodeMirror)
  - `DataGrid/` - Query results and table data display
  - `Sidebar/` - Connection/database tree navigation
  - `Metadata/` - Table structure and schema views
  - `SqlLogs/` - Query execution history
- `src/components/settings/` - Settings dialogs
- `src/services/` - Tauri API wrapper and mocks
- `src/lib/` - Utilities (i18n, keyboard shortcuts, validation)
- `src/theme/` - Theme registry and management

**Key Patterns:**
- All Tauri backend calls go through `src/services/api.ts` which provides:
  - Mock mode (`VITE_USE_MOCK=true`) for frontend-only development
  - Type-safe wrappers around Tauri `invoke()` commands
  - Runtime detection (`isTauri()`) to handle non-Tauri environments
- Path alias: `@/` maps to `./src/`
- i18n: Files in `src/lib/i18n/locales/` (en, zh, ja supported)

## Backend (Rust + Tauri)

**Core Modules:**
- `src-tauri/src/commands/` - Tauri command handlers (exposed to frontend):
  - `connection.rs` - Connection CRUD and testing
  - `query.rs` - Query execution and cancellation
  - `metadata.rs` - Schema inspection (tables, structures, DDL)
  - `storage.rs` - Saved queries persistence
  - `ai.rs` - AI provider management and chat
  - `transfer.rs` - Import/export operations
- `src-tauri/src/db/` - Database layer:
  - `drivers/` - Per-database implementations (postgres, mysql, clickhouse, mssql, sqlite, duckdb, oracle)
  - `pool_manager.rs` - Connection pooling with bb8
  - `local.rs` - SQLite database for app metadata
- `src-tauri/src/state.rs` - Global app state (local DB + pool manager)
- `src-tauri/src/ssh.rs` - SSH tunnel support
- `src-tauri/src/ai/` - AI provider integration (OpenAI-compatible APIs)
- `src-tauri/src/models/` - Shared data types
- `src-tauri/src/error.rs` - Error handling

**Key Patterns:**
- All database drivers implement `DatabaseDriver` trait (see `src-tauri/src/db/drivers/mod.rs`)
- Connection pooling: Each database connection gets a managed pool via `PoolManager`
- State management: `AppState` holds `local_db` (SQLite) and `pool_manager`
- SSH tunneling: Transparent port forwarding for remote database access
- Error messages: Use `conn_failed_error()` to provide context-aware hints (TLS issues, auth failures, network problems)

## Common Frontend-Backend Communication Pattern

```typescript
// Frontend
import { api } from '@/services/api';
const result = await api.execute_query(connectionId, database, sql);
```

```rust
// Backend
#[tauri::command]
async fn execute_query(
    state: State<'_, AppState>,
    connection_id: i64,
    database: Option<String>,
    sql: String,
) -> Result<QueryResult, String> {
    // Implementation
}
```

## Build System
- Frontend: Vite with React plugin and TailwindCSS
- Backend: Cargo with sqlx (compile-time SQL checking disabled), tiberius (SQL Server), bb8 (pooling)
- Platform toolchain: Follows Tauri v2 prerequisites (see https://tauri.app/start/prerequisites/)
- Package manager: Bun (preferred) or npm/pnpm

## CI/GitHub Actions
- `.github/workflows/ci.yml` - Main CI pipeline
- Tests run on: Ubuntu (Linux), macOS, Windows
- Integration tests use Docker containers (testcontainers)
- Release builds triggered on tags

## Mock Development
- Use `bun dev:mock` for rapid UI iteration without Rust compilation
- Mocks defined in `src/services/mocks.ts`
- Useful for working on: themes, UI components, layouts, i18n

## SSH Tunneling
- Handled transparently in connection layer
- SSH config in connection form, tunnel established before database connection
- Port forwarding lifetime managed with connection pool

## Translation/i18n
- Framework: i18next + react-i18next
- Locale files: `src/lib/i18n/locales/*.ts` (TypeScript, not JSON)
- Supported: English (en), Chinese (zh), Japanese (ja)
- To add language: Create locale file and register in `src/lib/i18n/index.ts`
