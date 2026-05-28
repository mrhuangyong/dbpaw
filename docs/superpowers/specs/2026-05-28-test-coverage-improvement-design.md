# Test Coverage Improvement Design

**Date:** 2026-05-28
**Scope:** Fix broken tests + add unit tests for frontend pure logic (v0.4.0+ features)

## Problem

v0.4.0 introduced 45 commits with several major features (ER Diagram, Db2 driver, Cassandra driver, Connection Import, MCP Server, multi-cell copy, etc.) but many lack unit tests. One existing test is also broken.

**Current state:**
- 571 frontend unit tests (24 files) — 1 failing
- 337 Rust unit tests — all passing
- 15 frontend service tests — all passing

## Goals

1. Fix the broken `driver-registry.unit.test.ts` test
2. Add unit tests for ER Diagram pure logic (`buildDiagramData`, `computeLayout`)
3. Extract multi-cell copy/format functions from `TableView.tsx` into testable pure functions and add tests
4. Add direct tests for `normalizeImportDriver`

**Non-goals:** Component tests, integration tests, Rust test improvements (separate effort).

## Changes

### 1. Fix driver-registry tests

**File:** `src/lib/driver-registry.unit.test.ts`

The test asserts 14 drivers but the registry now has 16 (`db2` and `cassandra` were added). Multiple assertion lists are incomplete.

Fixes needed:
- `toHaveLength(14)` → `toHaveLength(16)`
- Add `db2` and `cassandra` to the driver ID assertions (line 21-34)
- `isFileBasedDriver` network drivers list: add `oracle`, `db2`, `redis`, `mongodb`, `cassandra`
- `getDefaultPort`: add `oracle` (1521), `db2` (50000), `redis` (6379), `mongodb` (27017), `cassandra` (9042)
- `supportsRoutines` noRoutines list: add `cassandra`
- `supportsCreateDatabase` true list: add `cassandra`
- `supportsSchemaBrowsing`: add `db2` to the true list (it supports schema browsing)
- `importCapability` "all other drivers are supported" test: add `db2` to supported list; add `redis`, `elasticsearch`, `mongodb`, `cassandra` as separate "unsupported" test (they are not in the "supported" list)

### 2. ER Diagram unit tests

**New file:** `src/components/business/ERDiagram/types.unit.test.ts`

Tests for `buildDiagramData(overview, foreignKeys)`:

| Test case | Description |
|-----------|-------------|
| empty overview and FKs | Returns empty nodes and edges |
| no FK-related columns | Tables without FK columns are filtered out |
| FK source columns flagged | `isForeignKey: true` for source columns |
| FK target columns included | Target columns present but `isForeignKey: false` |
| schema resolution via fk.sourceSchema | Uses `fk.sourceSchema` when present |
| schema resolution via schemaByTable | Falls back to table's schema from overview |
| schema resolution fallback to "public" | Uses `"public"` when neither found |
| deterministic edge IDs | Edge ID format: `{name}-{srcTable}.{srcCol}-{tgtTable}.{tgtCol}` |
| multiple FKs between same tables | Multiple edges produced |
| self-referential FK | Table references itself, both source and target in same node |
| onUpdate/onDelete passed through | Values appear on edges |
| node ID format | `{schema}.{table}` |

**New file:** `src/components/business/ERDiagram/erDiagramLayout.unit.test.ts`

Tests for `computeLayout(nodes, edges)` (uses real dagre, verifies output properties):

| Test case | Description |
|-----------|-------------|
| nodes get positions | All nodes have x/y after layout |
| node height scales with columns | More columns = taller node |
| empty graph | Returns empty nodes/edges |
| single node | Positioned correctly |
| edges preserved | Output edges match input edges |

### 3. Extract and test multi-cell copy functions

**New file:** `src/components/business/DataGrid/tableView/selectionCopy.ts`

Extract these pure functions from `TableView.tsx`:

```typescript
// Normalize anchor/tip into min/max row/col
export function getNormalizedCellRange(
  anchor: { row: number; colIndex: number },
  tip: { row: number; colIndex: number },
): { minRow: number; maxRow: number; minCol: number; maxCol: number }

// Build TSV from cell range
export function buildRangeTSV(
  range: { minRow: number; maxRow: number; minCol: number; maxCol: number },
  columns: string[],
  rows: Record<string, any>[],
  getCellValue: (row: number, col: string, raw: any) => any,
  cellValueToString: (v: any) => string,
): string

// Build CSV from cell range (with proper escaping)
export function buildRangeCSV(
  range: { minRow: number; maxRow: number; minCol: number; maxCol: number },
  columns: string[],
  rows: Record<string, any>[],
  getCellValue: (row: number, col: string, raw: any) => any,
  cellValueToString: (v: any) => string,
): string

// Build INSERT SQL from cell range
export function buildRangeInsertSQL(
  range: { minRow: number; maxRow: number; minCol: number; maxCol: number },
  columns: string[],
  rows: Record<string, any>[],
  getCellValue: (row: number, col: string, raw: any) => any,
  formatSQLValue: (str: string, raw: any, mode: string, driver: string) => string,
  quoteIdent: (driver: string, ident: string) => string,
  driver: string,
  tableName: string,
): string

// Build UPDATE SQL from cell range
export function buildRangeUpdateSQL(
  range: { minRow: number; maxRow: number; minCol: number; maxCol: number },
  columns: string[],
  rows: Record<string, any>[],
  primaryKeys: string[],
  getCellValue: (row: number, col: string, raw: any) => any,
  formatSQLValue: (str: string, raw: any, mode: string, driver: string) => string,
  quoteIdent: (driver: string, ident: string) => string,
  escapeSQL: (s: string) => string,
  buildUpdateStatement: (driver: string, table: string, set: string, where: string) => string,
  driver: string,
  tableName: string,
): string

// Build TSV from row indexes
export function buildRowsTSV(
  rowIndexes: number[],
  columns: string[],
  rows: Record<string, any>[],
  getCellValue: (row: number, col: string, raw: any) => any,
  cellValueToString: (v: any) => string,
): string

// Build CSV from row indexes
export function buildRowsCSV(
  rowIndexes: number[],
  columns: string[],
  rows: Record<string, any>[],
  getCellValue: (row: number, col: string, raw: any) => any,
  cellValueToString: (v: any) => string,
): string

// Build INSERT SQL from row indexes
export function buildRowsInsertSQL(
  rowIndexes: number[],
  columns: string[],
  rows: Record<string, any>[],
  getCellValue: (row: number, col: string, raw: any) => any,
  formatSQLValue: (str: string, raw: any, mode: string, driver: string) => string,
  quoteIdent: (driver: string, ident: string) => string,
  driver: string,
  tableName: string,
): string

// Build UPDATE SQL from row indexes
export function buildRowsUpdateSQL(
  rowIndexes: number[],
  columns: string[],
  rows: Record<string, any>[],
  primaryKeys: string[],
  getCellValue: (row: number, col: string, raw: any) => any,
  formatSQLValue: (str: string, raw: any, mode: string, driver: string) => string,
  quoteIdent: (driver: string, ident: string) => string,
  escapeSQL: (s: string) => string,
  buildUpdateStatement: (driver: string, table: string, set: string, where: string) => string,
  driver: string,
  tableName: string,
): string
```

**New file:** `src/components/business/DataGrid/tableView/selectionCopy.unit.test.ts`

| Function | Test cases |
|----------|-----------|
| `getNormalizedCellRange` | normal range, inverted anchor/tip, same cell, null handling |
| `buildRangeTSV` | empty range, single cell, multi-cell, null values |
| `buildRangeCSV` | empty, single, multi, comma/quote/newline escaping |
| `buildRangeInsertSQL` | empty, single row, multi-row, proper quoting |
| `buildRangeUpdateSQL` | empty, no PKs, single row, multi-row, NULL PK handling |
| `buildRowsTSV` | empty, single, multi, null handling |
| `buildRowsCSV` | empty, single, multi, escaping |
| `buildRowsInsertSQL` | empty, single, multi |
| `buildRowsUpdateSQL` | no PKs, single, multi |

**Modified file:** `src/components/business/DataGrid/TableView.tsx`

Replace the inline `useCallback` implementations with calls to the extracted functions. No behavior change.

### 4. Connection import tests

**Modified file:** `src/services/api.unit.test.ts`

Add direct tests for `normalizeImportDriver`:

| Test case | Input → Output |
|-----------|---------------|
| postgresql → postgres | `"postgresql"` → `"postgres"` |
| pgsql → postgres | `"pgsql"` → `"postgres"` |
| mysql passthrough | `"mysql"` → `"mysql"` |
| empty string | `""` → `""` |
| whitespace trimmed | `" postgresql "` → `"postgres"` |
| case insensitive | `"PostgreSQL"` → `"postgres"` |

## Verification

After all changes:
1. `bun run test:unit` — all tests pass, 0 failures
2. `bun run test:service` — all tests pass
3. `bun run typecheck` — no type errors
4. `bun run rust:check` — no Rust errors (TableView.tsx changes are TS only)

## File Summary

| File | Action |
|------|--------|
| `src/lib/driver-registry.unit.test.ts` | Modify (fix assertions) |
| `src/components/business/ERDiagram/types.unit.test.ts` | Create |
| `src/components/business/ERDiagram/erDiagramLayout.unit.test.ts` | Create |
| `src/components/business/DataGrid/tableView/selectionCopy.ts` | Create (extract from TableView.tsx) |
| `src/components/business/DataGrid/tableView/selectionCopy.unit.test.ts` | Create |
| `src/components/business/DataGrid/TableView.tsx` | Modify (delegate to selectionCopy.ts) |
| `src/services/api.unit.test.ts` | Modify (add normalizeImportDriver tests) |
