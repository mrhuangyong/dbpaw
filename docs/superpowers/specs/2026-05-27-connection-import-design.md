# Connection Import — DBeaver & Navicat

## Overview

Add the ability to import database connection configurations from DBeaver (`data-sources.json`) and Navicat (`.ncx`) into DbPaw. Import-only — no export support.

## Scope

- **DBeaver**: Parse `data-sources.json` (workspace6 format)
- **Navicat**: Parse `.ncx` XML export files
- **Passwords**: Ignored — user fills in manually after import
- **Database types**: Only import types supported by DbPaw (16 drivers); skip unsupported types
- **Conflicts**: Auto-rename duplicates with `(1)`, `(2)` suffixes
- **Post-import**: Save directly to SQLite, no preview/test step

## Data Flow

```
User clicks "Import Connections" button
  → System file dialog (.json / .ncx filter)
  → Tauri command: import_connections(file_path)
    → Detect format by extension
    → Parse into Vec<ConnectionForm>
    → Map type identifiers to DbPaw driver names
    → Filter unsupported types (count as skipped)
    → Deduplicate names (auto-rename)
    → Batch insert into SQLite via LocalDb::create_connection()
  → Return ImportResult { imported, skipped }
  → Frontend toast + refresh connection list
```

## Type Mapping

### DBeaver provider → DbPaw driver

| DBeaver provider | DbPaw driver |
|---|---|
| `postgresql` | `postgres` |
| `mysql` | `mysql` |
| `mariadb` | `mariadb` |
| `tidb` | `tidb` |
| `sqlite` | `sqlite` |
| `duckdb` | `duckdb` |
| `clickhouse` | `clickhouse` |
| `sqlserver` | `mssql` |
| `oracle` | `oracle` |
| `db2` | `db2` |
| `redis` | `redis` |
| `elasticsearch` | `elasticsearch` |
| `mongodb` | `mongodb` |
| `cassandra` | `cassandra` |
| `starrocks` | `starrocks` |
| `doris` | `doris` |

All other providers → skipped.

### Navicat ConnType → DbPaw driver

| Navicat ConnType | DbPaw driver |
|---|---|
| `MYSQL` | `mysql` |
| `MARIADB` | `mariadb` |
| `POSTGRESQL` | `postgres` |
| `ORACLE` | `oracle` |
| `SQLITE` | `sqlite` |
| `MSSQL` | `mssql` |
| `MONGODB` | `mongodb` |
| `REDIS` | `redis` |
| `CLICKHOUSE` | `clickhouse` |

All other types → skipped.

### Field Mapping

| Source field | ConnectionForm field | Notes |
|---|---|---|
| host / Host | host | Default to `localhost` if empty |
| port / Port | port | Default handled by normalize_connection_form |
| database / DatabaseName | database | |
| user / UserName | username | |
| name / ConnectionName | name | Default to `"<driver> - <host>:<port>"` if empty |
| SSL | ssl | |
| SSHHost | ssh_host | |
| SSHPort | ssh_port | |
| SSHUserName | ssh_username | |
| SSHKeyFile | ssh_key_path | |
| Password | — | Ignored |

## UI Design

### Entry Point

Import button in sidebar header, left of the refresh button:

```
┌──────────────────────────────────┐
│  Connections     [↑] [↻] [+]    │  ← ↑ = Import button (Upload icon)
├──────────────────────────────────┤
│  🔍 Search tables...             │
├──────────────────────────────────┤
│  ...                             │
```

### Interaction

1. Click import → `open()` file dialog with filters: `DBeaver JSON (*.json), Navicat NCX (*.ncx)`
2. User selects file → calls `api.connections.importFromFile(filePath)`
3. Backend parses + batch creates → returns `ImportResult`
4. Toast feedback:
   - Success: `"Successfully imported N connections"` (if skipped: `"Skipped M unsupported types"`)
   - Failure: `"Import failed: <error message>"`
5. Auto-refresh connection list

### TypeScript Types

```typescript
interface ImportResult {
  imported: SavedConnection[];
  skipped: number;
}
```

### i18n Keys

- `connection.import.title`: "Import Connections"
- `connection.toast.importSuccess`: "Successfully imported {{count}} connections"
- `connection.toast.importSkipped`: "Skipped {{count}} unsupported types"
- `connection.toast.importFailed`: "Import failed"

## Rust Implementation

### New Module Structure

```
src-tauri/src/import/
├── mod.rs          # Entry: detect_format(), import_from_file()
├── dbeaver.rs      # DBeaver data-sources.json parser
└── navicat.rs      # Navicat .ncx XML parser
```

### `import/mod.rs`

```rust
pub enum ImportFormat { DBeaver, Navicat }

pub struct ImportResult {
    pub imported: Vec<Connection>,
    pub skipped: usize,
}

pub fn detect_format(path: &str) -> Result<ImportFormat, String>
pub async fn import_from_file(path: &str, local_db: &LocalDb) -> Result<ImportResult, String>
```

- `.json` extension → try DBeaver parse, error if fails
- `.ncx` extension → Navicat parse
- Other → error with message about supported formats

### `import/dbeaver.rs`

```rust
pub fn parse_dbeaver_json(content: &str) -> Result<Vec<ConnectionForm>, String>
```

- Parse top-level JSON object, each key is a connection
- Read `provider` → map to driver
- Extract `configuration.host`, `configuration.port`, `configuration.database`, `configuration.user`
- Ignore `configuration.password`
- SSH/SSL fields not available in DBeaver's data-sources.json → left empty

### `import/navicat.rs`

```rust
pub fn parse_navicat_ncx(content: &str) -> Result<Vec<ConnectionForm>, String>
```

- Parse `<Connections>` → `<Connection>` elements with `quick-xml`
- Read attributes: `ConnType`, `Host`, `Port`, `DatabaseName`, `UserName`, `ConnectionName`
- SSH: `SSHHost`, `SSHPort`, `SSHUserName`, `SSHKeyFile`
- SSL: `SSL` attribute
- Ignore password

### Tauri Command

In `commands/connection.rs`:

```rust
#[tauri::command]
pub async fn import_connections(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<ImportResult, String>
```

Register in `lib.rs` invoke_handler.

### Cargo Dependency

Add `quick-xml = "0.37"` to `Cargo.toml` for Navicat NCX parsing.

## Error Handling

| Scenario | Handling |
|---|---|
| File not found / no permission | `Err("Cannot read file: <reason>")` |
| JSON parse error | `Err("DBeaver JSON parse failed: <reason>")` |
| XML parse error | `Err("Navicat NCX parse failed: <reason>")` |
| All connections unsupported type | `ImportResult { imported: [], skipped: N }` + toast |
| Partial creation failure | Successful ones saved, last error returned |

## Edge Cases

- **Duplicate names**: Query existing names before insert, append ` (1)`, ` (2)` etc. until unique
- **Empty host**: Default to `localhost`
- **Empty port**: Handled by existing `normalize_connection_form` (driver defaults)
- **Empty database**: Allowed — some databases allow connecting without specifying one
- **Empty name**: Default to `"<driver> - <host>:<port>"`

## Files to Modify

| File | Change |
|---|---|
| `src-tauri/Cargo.toml` | Add `quick-xml` dependency |
| `src-tauri/src/import/mod.rs` | New — format detection + import entry point |
| `src-tauri/src/import/dbeaver.rs` | New — DBeaver JSON parser |
| `src-tauri/src/import/navicat.rs` | New — Navicat NCX parser |
| `src-tauri/src/main.rs` or `lib.rs` | Add `mod import;` declaration |
| `src-tauri/src/commands/connection.rs` | Add `import_connections` Tauri command |
| `src-tauri/src/lib.rs` | Register `import_connections` in invoke_handler |
| `src/services/api.ts` | Add `api.connections.importFromFile()` method |
| `src/services/mocks.ts` | Add mock for `importFromFile` |
| `src/components/business/Sidebar/ConnectionList.tsx` | Add import button + handler |
| `src/lib/i18n/locales/en.ts` | Add i18n keys |
| `src/lib/i18n/locales/zh.ts` | Add i18n keys |
