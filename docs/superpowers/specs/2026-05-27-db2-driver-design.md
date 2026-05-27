# IBM Db2 LUW Driver Design

## Overview

Add IBM Db2 LUW (Linux/Unix/Windows) database support to DbPaw as a new driver. Users can connect to Db2 11.5+ instances, browse schemas, inspect tables, run queries, and manage routines — full parity with existing drivers.

## Requirements

- **Variant:** Db2 LUW only (not z/OS or Db2 for i)
- **Connection:** ODBC via the `odbc-api` Rust crate
- **Scope:** Full `DatabaseDriver` trait implementation (all required + optional methods)
- **Testing:** Docker-based integration tests with `icr.io/db2_community/db2`
- **Prerequisite:** Users must have the IBM Db2 ODBC driver installed; error messages guide installation if missing

## Architecture

### New File

`src-tauri/src/db/drivers/db2.rs` — implements `DatabaseDriver` trait. Struct `Db2Driver` holds an `odbc-api` connection handle.

### Registration (mod.rs)

Three changes in `src-tauri/src/db/drivers/mod.rs`:

1. Add `pub mod db2;` declaration
2. Add `use self::db2::Db2Driver;` import
3. Add `"db2"` match arm in `connect()` function

### Cargo.toml

Add `odbc-api` dependency. This pulls in `odbc-sys` transitively.

## Connection Configuration

### Connection Form Fields

| Field | Default | Notes |
|---|---|---|
| host | — | Db2 server hostname |
| port | 50000 | Default Db2 instance port |
| database | — | Maps to `DATABASE` in ODBC string |
| username | — | Standard auth |
| password | — | Standard auth |
| SSH tunnel | — | Transparent via existing infrastructure |

### ODBC Connection String

```
DRIVER={IBM DB2 ODBC DRIVER};DATABASE={db};HOSTNAME={host};PORT={port};PROTOCOL=TCPIP;UID={user};PWD={pass};
```

The driver name may vary by platform (`IBM DB2 ODBC DRIVER` on Linux, `DB2` on macOS via iODBC). Support both common names with fallback error message.

## SQL Queries (System Catalog)

Db2 uses `SYSCAT` system catalog views:

### List Databases

Db2 LUW connections are database-specific (you connect to one database). `list_databases()` returns a single-element vec containing the connected database name. To discover other databases on the instance, users create separate connections.

### List Tables

```sql
SELECT TABNAME, TYPE, REMARKS
FROM SYSCAT.TABLES
WHERE TABSCHEMA = ?
  AND TYPE IN ('T', 'V')
ORDER BY TABNAME
```

### Table Columns

```sql
SELECT COLNAME, TYPENAME, LENGTH, SCALE, NULLS, DEFAULT, REMARKS
FROM SYSCAT.COLUMNS
WHERE TABSCHEMA = ? AND TABNAME = ?
ORDER BY COLNO
```

### Table DDL

Db2 has no native `GET_DDL` function. Generate DDL from catalog metadata:

1. Query `SYSCAT.TABLES` for table type
2. Query `SYSCAT.COLUMNS` for column definitions
3. Query `SYSCAT.INDEXES` for indexes
4. Query `SYSCAT.TABCONST` and `SYSCAT.CONSTDEP` for constraints
5. Construct `CREATE TABLE` statement programmatically

### Routines

```sql
SELECT ROUTINENAME, ROUTINETYPE, SPECIFICNAME
FROM SYSCAT.ROUTINES
WHERE ROUTINESCHEMA = ?
ORDER BY ROUTINENAME
```

Routine DDL via `SYSCAT.ROUTINES.TEXT` column.

### Schema Overview

```sql
SELECT TABSCHEMA, TABNAME, TYPE, CARD, STATS_TIME
FROM SYSCAT.TABLES
WHERE TABSCHEMA NOT LIKE 'SYS%'
ORDER BY TABSCHEMA, TABNAME
```

## Frontend Changes

### Driver Registry (`src/lib/driver-registry.tsx`)

Add `db2` entry:
- ID: `db2`
- Default port: `50000`
- Icon: Db2 logo (SVG)
- Capabilities: SQL, routines, schema overview
- Connection fields: host, port, database, username, password

### i18n

Add Db2 strings to `src/lib/i18n/locales/en.ts`, `zh.ts`, `ja.ts`.

### API Layer

No changes to `src/services/api.ts` — uses generic connection API.

### Connection Validation (`src-tauri/src/connection_input/mod.rs`)

Add `db2` to driver validation match arms in `normalize_connection_form()`.

## Error Handling

Use `conn_failed_error()` from `mod.rs` for connection failures. Db2-specific hints:

| Error | Hint |
|---|---|
| ODBC driver not found | "Install the IBM Db2 ODBC driver: https://www.ibm.com/docs/en/db2/11.5?topic=clients" |
| Connection refused | "Check that the Db2 instance is running on port {port}" |
| Auth failure | "Verify your Db2 username and password" |
| Database not found | "Check the database name. Use `db2 list db directory` to list available databases" |

## Testing

### Docker Image

`icr.io/db2_community/db2:11.5.9.0` — IBM's official community edition.

Environment variables:
- `LICENSE=accept`
- `DB2INST1_PASSWORD=testpass`
- `DBNAME=testdb`

Startup time: 60-90 seconds (longer than other DB containers).

### Test Files

1. `tests/db2_integration.rs` — direct driver method tests
2. `tests/db2_command_integration.rs` — ephemeral connection commands
3. `tests/db2_stateful_command_integration.rs` — saved connection workflows

All added to `scripts/test-integration.sh` with `IT_DB=db2`.

### Test Queries

- Schema listing and table browsing
- Column metadata and types
- DDL generation verification
- CRUD operations via `execute_query`
- Routine listing

## Implementation Steps

1. Add `odbc-api` to `src-tauri/Cargo.toml`
2. Create `src-tauri/src/db/drivers/db2.rs` implementing `DatabaseDriver`
3. Register in `mod.rs` (pub mod, use, connect match arm)
4. Add SSH default port in `ssh.rs`
5. Update connection form validation in `connection_input/mod.rs`
6. Add import/export syntax in `commands/transfer.rs`
7. Update `src/lib/driver-registry.tsx`
8. Add i18n strings
9. Create integration test files
10. Update `scripts/test-integration.sh`

## Open Questions

None — all decisions made during brainstorming.
