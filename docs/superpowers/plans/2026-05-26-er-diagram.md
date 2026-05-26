# ER Diagram Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an interactive ER diagram that visualizes all tables and their foreign key relationships in the current database/schema.

**Architecture:** Backend provides a new `get_schema_foreign_keys` command that returns all FK relationships for a schema. Frontend uses React Flow + dagre to render an interactive, auto-laid-out graph of tables connected by FK edges.

**Tech Stack:** Rust (sqlx, serde), TypeScript, React, @xyflow/react (React Flow v12), dagre

---

## File Structure

### Backend (Rust)

| File | Change |
|------|--------|
| `src-tauri/src/models/mod.rs` | Add `SchemaForeignKey` struct |
| `src-tauri/src/db/drivers/mod.rs` | Add `get_schema_foreign_keys` default method to `DatabaseDriver` trait |
| `src-tauri/src/db/drivers/postgres.rs` | Implement `get_schema_foreign_keys` |
| `src-tauri/src/db/drivers/mysql.rs` | Implement `get_schema_foreign_keys` |
| `src-tauri/src/db/drivers/mssql.rs` | Implement `get_schema_foreign_keys` |
| `src-tauri/src/db/drivers/oracle.rs` | Implement `get_schema_foreign_keys` |
| `src-tauri/src/db/drivers/sqlite.rs` | Implement `get_schema_foreign_keys` |
| `src-tauri/src/commands/metadata.rs` | Add `get_schema_foreign_keys` command |
| `src-tauri/src/lib.rs` | Register new command in `generate_handler!` |

### Frontend (TypeScript/React)

| File | Change |
|------|--------|
| `package.json` | Add `@xyflow/react`, `dagre`, `@types/dagre` |
| `src/services/api.ts` | Add `getSchemaForeignKeys` + `SchemaForeignKey` interface |
| `src/services/mocks.ts` | Add mock implementation |
| `src/components/business/ERDiagram/types.ts` | ER diagram type definitions |
| `src/components/business/ERDiagram/erDiagramLayout.ts` | dagre layout computation |
| `src/components/business/ERDiagram/TableNode.tsx` | Custom React Flow node |
| `src/components/business/ERDiagram/ERDiagramCanvas.tsx` | React Flow canvas |
| `src/components/business/ERDiagram/ERDiagramView.tsx` | Main container with data fetching |
| `src/App.tsx` | Add `"er-diagram"` tab type + toolbar button |
| `src/lib/i18n/locales/en.ts` | Add i18n keys |
| `src/lib/i18n/locales/zh.ts` | Add i18n keys |

---

## Task 1: Backend — Add SchemaForeignKey type and command

### Files
- Modify: `src-tauri/src/models/mod.rs:410`
- Modify: `src-tauri/src/commands/metadata.rs:199`
- Modify: `src-tauri/src/lib.rs:159`

### Steps

- [ ] **Step 1: Add SchemaForeignKey struct to models**

In `src-tauri/src/models/mod.rs`, add after the `SchemaOverview` struct (after line 410):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaForeignKey {
    pub name: String,
    pub source_table: String,
    pub source_schema: Option<String>,
    pub source_column: String,
    pub target_table: String,
    pub target_schema: Option<String>,
    pub target_column: String,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}
```

- [ ] **Step 2: Add get_schema_foreign_keys to DatabaseDriver trait**

In `src-tauri/src/db/drivers/mod.rs`, add after `get_schema_overview` (after line 373):

```rust
async fn get_schema_foreign_keys(
    &self,
    _database: Option<&str>,
) -> Result<Vec<SchemaForeignKey>, String> {
    Ok(vec![])
}
```

Add `SchemaForeignKey` to the imports at the top of the file.

- [ ] **Step 3: Add Tauri command**

In `src-tauri/src/commands/metadata.rs`, add at the end of the file:

```rust
#[tauri::command]
pub async fn get_schema_foreign_keys(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
) -> Result<Vec<SchemaForeignKey>, String> {
    super::execute_with_retry(&state, id, database, |driver| {
        async move { driver.get_schema_foreign_keys(None).await }
    })
    .await
}
```

Add `SchemaForeignKey` to the imports at line 1-3.

- [ ] **Step 4: Register command in lib.rs**

In `src-tauri/src/lib.rs`, add to the `generate_handler!` array after `get_schema_overview` (line 159):

```rust
commands::metadata::get_schema_foreign_keys,
```

- [ ] **Step 5: Verify Rust compiles**

Run: `cargo check` from `src-tauri/`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models/mod.rs src-tauri/src/db/drivers/mod.rs src-tauri/src/commands/metadata.rs src-tauri/src/lib.rs
git commit -m "feat(er): add SchemaForeignKey type and get_schema_foreign_keys command"
```

---

## Task 2: Backend — Implement driver-specific FK queries

### Files
- Modify: `src-tauri/src/db/drivers/postgres.rs`
- Modify: `src-tauri/src/db/drivers/mysql.rs`
- Modify: `src-tauri/src/db/drivers/mssql.rs`
- Modify: `src-tauri/src/db/drivers/oracle.rs`
- Modify: `src-tauri/src/db/drivers/sqlite.rs`

### Steps

- [ ] **Step 1: Implement for Postgres**

In `src-tauri/src/db/drivers/postgres.rs`, add the method implementation inside the `impl DatabaseDriver for PostgresDriver` block:

```rust
async fn get_schema_foreign_keys(
    &self,
    _database: Option<&str>,
) -> Result<Vec<SchemaForeignKey>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
          con.conname AS constraint_name,
          n.nspname AS source_schema,
          c.relname AS source_table,
          a.attname AS source_column,
          fn.nspname AS target_schema,
          fc.relname AS target_table,
          fa.attname AS target_column,
          CASE con.confupdtype::text
            WHEN 'a' THEN 'NO ACTION'
            WHEN 'r' THEN 'RESTRICT'
            WHEN 'c' THEN 'CASCADE'
            WHEN 'n' THEN 'SET NULL'
            WHEN 'd' THEN 'SET DEFAULT'
            ELSE NULL
          END AS on_update,
          CASE con.confdeltype::text
            WHEN 'a' THEN 'NO ACTION'
            WHEN 'r' THEN 'RESTRICT'
            WHEN 'c' THEN 'CASCADE'
            WHEN 'n' THEN 'SET NULL'
            WHEN 'd' THEN 'SET DEFAULT'
            ELSE NULL
          END AS on_delete
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_class fc ON fc.oid = con.confrelid
        JOIN pg_namespace fn ON fn.oid = fc.relnamespace
        JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS ck(attnum, ord) ON true
        JOIN LATERAL unnest(con.confkey) WITH ORDINALITY AS fk(attnum, ord) ON fk.ord = ck.ord
        JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ck.attnum
        JOIN pg_attribute fa ON fa.attrelid = fc.oid AND fa.attnum = fk.attnum
        WHERE con.contype = 'f'
        ORDER BY con.conname, ck.ord
        "#,
    )
    .fetch_all(&self.pool)
    .await
    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

    let mut foreign_keys = Vec::new();
    for row in rows {
        let source_schema: String = row.try_get(1).unwrap_or_default();
        let target_schema: String = row.try_get(4).unwrap_or_default();
        foreign_keys.push(SchemaForeignKey {
            name: row.try_get(0).unwrap_or_default(),
            source_schema: if source_schema.is_empty() { None } else { Some(source_schema) },
            source_table: row.try_get(2).unwrap_or_default(),
            source_column: row.try_get(3).unwrap_or_default(),
            target_schema: if target_schema.is_empty() { None } else { Some(target_schema) },
            target_table: row.try_get(5).unwrap_or_default(),
            target_column: row.try_get(6).unwrap_or_default(),
            on_update: row.try_get::<Option<String>, _>(7).unwrap_or(None),
            on_delete: row.try_get::<Option<String>, _>(8).unwrap_or(None),
        });
    }
    Ok(foreign_keys)
}
```

Add `SchemaForeignKey` to the imports at the top of the file.

- [ ] **Step 2: Implement for MySQL**

In `src-tauri/src/db/drivers/mysql.rs`, add the method:

```rust
async fn get_schema_foreign_keys(
    &self,
    _database: Option<&str>,
) -> Result<Vec<SchemaForeignKey>, String> {
    let db_name = _database.unwrap_or_else(|| "public".to_string());
    let rows = sqlx::query(
        r#"
        SELECT
          kcu.CONSTRAINT_NAME,
          kcu.TABLE_SCHEMA,
          kcu.TABLE_NAME,
          kcu.COLUMN_NAME,
          kcu.REFERENCED_TABLE_SCHEMA,
          kcu.REFERENCED_TABLE_NAME,
          kcu.REFERENCED_COLUMN_NAME,
          rc.UPDATE_RULE,
          rc.DELETE_RULE
        FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu
        JOIN INFORMATION_SCHEMA.REFERENTIAL_CONSTRAINTS rc
          ON kcu.CONSTRAINT_NAME = rc.CONSTRAINT_NAME
          AND kcu.TABLE_SCHEMA = rc.CONSTRAINT_SCHEMA
        WHERE kcu.REFERENCED_TABLE_NAME IS NOT NULL
        ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION
        "#,
    )
    .fetch_all(&self.pool)
    .await
    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

    let mut foreign_keys = Vec::new();
    for row in rows {
        let source_schema: String = row.try_get(1).unwrap_or_default();
        let target_schema: String = row.try_get(4).unwrap_or_default();
        foreign_keys.push(SchemaForeignKey {
            name: row.try_get(0).unwrap_or_default(),
            source_schema: Some(source_schema),
            source_table: row.try_get(2).unwrap_or_default(),
            source_column: row.try_get(3).unwrap_or_default(),
            target_schema: Some(target_schema),
            target_table: row.try_get(5).unwrap_or_default(),
            target_column: row.try_get(6).unwrap_or_default(),
            on_update: row.try_get(7).unwrap_or(None),
            on_delete: row.try_get(8).unwrap_or(None),
        });
    }
    Ok(foreign_keys)
}
```

Add `SchemaForeignKey` to the imports.

- [ ] **Step 3: Implement for MSSQL**

In `src-tauri/src/db/drivers/mssql.rs`, add the method:

```rust
async fn get_schema_foreign_keys(
    &self,
    _database: Option<&str>,
) -> Result<Vec<SchemaForeignKey>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
          fk.name AS constraint_name,
          ss.name AS source_schema,
          ts.name AS source_table,
          cp.name AS source_column,
          rs.name AS target_schema,
          tr.name AS target_table,
          cr.name AS target_column,
          NULL AS on_update,
          NULL AS on_delete
        FROM sys.foreign_keys fk
        JOIN sys.foreign_key_columns fkc ON fk.object_id = fkc.constraint_object_id
        JOIN sys.tables ts ON fkc.parent_object_id = ts.object_id
        JOIN sys.schemas ss ON ts.schema_id = ss.schema_id
        JOIN sys.columns cp ON fkc.parent_object_id = cp.object_id AND fkc.parent_column_id = cp.column_id
        JOIN sys.tables tr ON fkc.referenced_object_id = tr.object_id
        JOIN sys.schemas rs ON tr.schema_id = rs.schema_id
        JOIN sys.columns cr ON fkc.referenced_object_id = cr.object_id AND fkc.referenced_column_id = cr.column_id
        ORDER BY fk.name, fkc.constraint_column_id
        "#,
    )
    .fetch_all(&self.pool)
    .await
    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

    let mut foreign_keys = Vec::new();
    for row in rows {
        let source_schema: String = row.try_get(1).unwrap_or_default();
        let target_schema: String = row.try_get(4).unwrap_or_default();
        foreign_keys.push(SchemaForeignKey {
            name: row.try_get(0).unwrap_or_default(),
            source_schema: Some(source_schema),
            source_table: row.try_get(2).unwrap_or_default(),
            source_column: row.try_get(3).unwrap_or_default(),
            target_schema: Some(target_schema),
            target_table: row.try_get(5).unwrap_or_default(),
            target_column: row.try_get(6).unwrap_or_default(),
            on_update: row.try_get(7).unwrap_or(None),
            on_delete: row.try_get(8).unwrap_or(None),
        });
    }
    Ok(foreign_keys)
}
```

Add `SchemaForeignKey` to the imports.

- [ ] **Step 4: Implement for Oracle**

In `src-tauri/src/db/drivers/oracle.rs`, add the method:

```rust
async fn get_schema_foreign_keys(
    &self,
    _database: Option<&str>,
) -> Result<Vec<SchemaForeignKey>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
          ac.constraint_name,
          acc_src.table_name AS source_table,
          acc_src.column_name AS source_column,
          acc_tgt.table_name AS target_table,
          acc_tgt.column_name AS target_column,
          ac.delete_rule
        FROM user_constraints ac
        JOIN user_cons_columns acc_src ON ac.constraint_name = acc_src.constraint_name
        JOIN user_cons_columns acc_tgt ON ac.r_constraint_name = acc_tgt.constraint_name
          AND acc_src.position = acc_tgt.position
        WHERE ac.constraint_type = 'R'
        ORDER BY ac.constraint_name, acc_src.position
        "#,
    )
    .fetch_all(&self.pool)
    .await
    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

    let mut foreign_keys = Vec::new();
    for row in rows {
        foreign_keys.push(SchemaForeignKey {
            name: row.try_get(0).unwrap_or_default(),
            source_schema: None,
            source_table: row.try_get(1).unwrap_or_default(),
            source_column: row.try_get(2).unwrap_or_default(),
            target_schema: None,
            target_table: row.try_get(3).unwrap_or_default(),
            target_column: row.try_get(4).unwrap_or_default(),
            on_update: None,
            on_delete: row.try_get(5).unwrap_or(None),
        });
    }
    Ok(foreign_keys)
}
```

Add `SchemaForeignKey` to the imports.

- [ ] **Step 5: Implement for SQLite**

In `src-tauri/src/db/drivers/sqlite.rs`, add the method:

```rust
async fn get_schema_foreign_keys(
    &self,
    _database: Option<&str>,
) -> Result<Vec<SchemaForeignKey>, String> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    )
    .fetch_all(&self.pool)
    .await
    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

    let mut foreign_keys = Vec::new();
    for table in tables {
        let rows = sqlx::query(&format!("PRAGMA foreign_key_list('{}')", table.replace('\'', "''")))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        for row in rows {
            let target_table: String = row.try_get(2).unwrap_or_default();
            let source_column: String = row.try_get(3).unwrap_or_default();
            let target_column: String = row.try_get(4).unwrap_or_default();
            let on_update: String = row.try_get(5).unwrap_or_default();
            let on_delete: String = row.try_get(6).unwrap_or_default();
            foreign_keys.push(SchemaForeignKey {
                name: format!("fk_{}_{}", table, source_column),
                source_schema: None,
                source_table: table.clone(),
                source_column,
                target_schema: None,
                target_table,
                target_column,
                on_update: if on_update.is_empty() { None } else { Some(on_update) },
                on_delete: if on_delete.is_empty() { None } else { Some(on_delete) },
            });
        }
    }
    Ok(foreign_keys)
}
```

Add `SchemaForeignKey` to the imports.

- [ ] **Step 6: Verify Rust compiles**

Run: `cargo check` from `src-tauri/`
Expected: Compiles without errors

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db/drivers/postgres.rs src-tauri/src/db/drivers/mysql.rs src-tauri/src/db/drivers/mssql.rs src-tauri/src/db/drivers/oracle.rs src-tauri/src/db/drivers/sqlite.rs
git commit -m "feat(er): implement get_schema_foreign_keys for all relational drivers"
```

---

## Task 3: Frontend — Install dependencies and add API layer

### Files
- Modify: `package.json`
- Modify: `src/services/api.ts:865`
- Modify: `src/services/mocks.ts`

### Steps

- [ ] **Step 1: Install React Flow and dagre**

Run:
```bash
bun add @xyflow/react dagre
bun add -D @types/dagre
```

Expected: `@xyflow/react` and `dagre` added to dependencies, `@types/dagre` to devDependencies

- [ ] **Step 2: Add SchemaForeignKey interface to api.ts**

In `src/services/api.ts`, add after the `TableMetadata` interface (around line 604):

```typescript
export interface SchemaForeignKey {
  name: string;
  sourceTable: string;
  sourceSchema?: string | null;
  sourceColumn: string;
  targetTable: string;
  targetSchema?: string | null;
  targetColumn: string;
  onUpdate?: string | null;
  onDelete?: string | null;
}
```

- [ ] **Step 3: Add getSchemaForeignKeys to api.metadata**

In `src/services/api.ts`, add inside the `metadata` object after `getSchemaOverview` (after line 864):

```typescript
getSchemaForeignKeys: (id: number, database?: string) =>
  invoke<SchemaForeignKey[]>("get_schema_foreign_keys", { id, database }),
```

- [ ] **Step 4: Add mock data and handler**

In `src/services/mocks.ts`, add mock data after `mockTableMetadata`:

```typescript
export const mockSchemaForeignKeys: SchemaForeignKey[] = [
  {
    name: "fk_user_role",
    sourceTable: "users",
    sourceColumn: "role_id",
    targetTable: "roles",
    targetColumn: "id",
    onUpdate: "CASCADE",
    onDelete: "SET NULL",
  },
  {
    name: "fk_order_user",
    sourceTable: "orders",
    sourceColumn: "user_id",
    targetTable: "users",
    targetColumn: "id",
    onUpdate: "NO ACTION",
    onDelete: "CASCADE",
  },
  {
    name: "fk_order_item_order",
    sourceTable: "order_items",
    sourceColumn: "order_id",
    targetTable: "orders",
    targetColumn: "id",
    onUpdate: "NO ACTION",
    onDelete: "CASCADE",
  },
];
```

Add mock handler function:

```typescript
export async function mockGetSchemaForeignKeys(
  _id: number,
  _database?: string,
): Promise<SchemaForeignKey[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockSchemaForeignKeys;
}
```

Add to `invokeMock` switch statement after the `get_schema_overview` case:

```typescript
case "get_schema_foreign_keys":
  return mockGetSchemaForeignKeys(args.id, args.database) as Promise<T>;
```

Import `SchemaForeignKey` type at the top.

- [ ] **Step 5: Verify frontend compiles**

Run: `bun run typecheck`
Expected: No type errors

- [ ] **Step 6: Commit**

```bash
git add package.json bun.lock src/services/api.ts src/services/mocks.ts
git commit -m "feat(er): install react-flow + dagre, add API layer and mocks"
```

---

## Task 4: Frontend — Create ERDiagram components

### Files
- Create: `src/components/business/ERDiagram/types.ts`
- Create: `src/components/business/ERDiagram/erDiagramLayout.ts`
- Create: `src/components/business/ERDiagram/TableNode.tsx`
- Create: `src/components/business/ERDiagram/ERDiagramCanvas.tsx`
- Create: `src/components/business/ERDiagram/ERDiagramView.tsx`

### Steps

- [ ] **Step 1: Create types.ts**

```typescript
import type { SchemaOverview, SchemaForeignKey } from "@/services/api";

export interface ERDiagramTableNode {
  id: string;
  schema: string;
  name: string;
  columns: {
    name: string;
    type: string;
    isPrimaryKey: boolean;
    isForeignKey: boolean;
  }[];
}

export interface ERDiagramEdge {
  id: string;
  source: string;
  target: string;
  sourceColumn: string;
  targetColumn: string;
  fkName: string;
  onUpdate?: string | null;
  onDelete?: string | null;
}

export interface ERDiagramData {
  nodes: ERDiagramTableNode[];
  edges: ERDiagramEdge[];
}

export function buildDiagramData(
  overview: SchemaOverview,
  foreignKeys: SchemaForeignKey[],
): ERDiagramData {
  const fkSourceSet = new Set<string>();
  const fkTargetSet = new Set<string>();
  const pkSet = new Set<string>();

  foreignKeys.forEach((fk) => {
    fkSourceSet.add(`${fk.sourceTable}.${fk.sourceColumn}`);
    fkTargetSet.add(`${fk.targetTable}.${fk.targetColumn}`);
  });

  const nodes: ERDiagramTableNode[] = overview.tables.map((table) => ({
    id: `${table.schema}.${table.name}`,
    schema: table.schema,
    name: table.name,
    columns: table.columns
      .filter((col) => {
        const key = `${table.name}.${col.name}`;
        return fkSourceSet.has(key) || fkTargetSet.has(key) || pkSet.has(key);
      })
      .map((col) => ({
        name: col.name,
        type: col.type,
        isPrimaryKey: pkSet.has(`${table.name}.${col.name}`),
        isForeignKey: fkSourceSet.has(`${table.name}.${col.name}`),
      })),
  }));

  const edges: ERDiagramEdge[] = foreignKeys.map((fk) => ({
    id: `${fk.sourceTable}.${fk.sourceColumn}-${fk.targetTable}.${fk.targetColumn}`,
    source: `${fk.sourceSchema || "public"}.${fk.sourceTable}`,
    target: `${fk.targetSchema || "public"}.${fk.targetTable}`,
    sourceColumn: fk.sourceColumn,
    targetColumn: fk.targetColumn,
    fkName: fk.name,
    onUpdate: fk.onUpdate,
    onDelete: fk.onDelete,
  }));

  return { nodes, edges };
}
```

- [ ] **Step 2: Create erDiagramLayout.ts**

```typescript
import dagre from "dagre";
import type { Node, Edge } from "@xyflow/react";

const NODE_WIDTH = 220;
const NODE_HEADER_HEIGHT = 40;
const NODE_ROW_HEIGHT = 28;
const NODESEP = 80;
const RANKSEP = 100;

export function computeLayout(
  nodes: Node[],
  edges: Edge[],
): { nodes: Node[]; edges: Edge[] } {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "TB", nodesep: NODESEP, ranksep: RANKSEP });

  nodes.forEach((node) => {
    const columnCount = (node.data?.columns as any[])?.length || 0;
    const height = NODE_HEADER_HEIGHT + columnCount * NODE_ROW_HEIGHT;
    g.setNode(node.id, { width: NODE_WIDTH, height });
  });

  edges.forEach((edge) => {
    g.setEdge(edge.source, edge.target);
  });

  dagre.layout(g);

  const layoutedNodes = nodes.map((node) => {
    const pos = g.node(node.id);
    return {
      ...node,
      position: { x: pos.x - NODE_WIDTH / 2, y: pos.y - (pos.height || 100) / 2 },
    };
  });

  return { nodes: layoutedNodes, edges };
}
```

- [ ] **Step 3: Create TableNode.tsx**

```tsx
import { memo } from "react";
import { Handle, Position } from "@xyflow/react";

interface ColumnData {
  name: string;
  type: string;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
}

interface TableNodeData {
  label: string;
  columns: ColumnData[];
  [key: string]: unknown;
}

function TableNode({ data }: { data: TableNodeData }) {
  return (
    <div className="rounded-lg border border-border bg-card shadow-md min-w-[200px]">
      <Handle type="target" position={Position.Top} className="!bg-transparent" />
      <div className="px-3 py-2 bg-primary/10 border-b border-border rounded-t-lg">
        <span className="font-semibold text-sm truncate block max-w-[180px]">
          {data.label}
        </span>
      </div>
      <div className="px-2 py-1">
        {data.columns.map((col) => (
          <div
            key={col.name}
            className="flex items-center gap-1.5 py-0.5 text-xs"
          >
            {col.isPrimaryKey && (
              <span className="inline-block w-4 text-center text-[10px] font-bold text-yellow-500 bg-yellow-500/10 rounded px-0.5">
                PK
              </span>
            )}
            {col.isForeignKey && !col.isPrimaryKey && (
              <span className="inline-block w-4 text-center text-[10px] font-bold text-blue-500 bg-blue-500/10 rounded px-0.5">
                FK
              </span>
            )}
            {!col.isPrimaryKey && !col.isForeignKey && (
              <span className="inline-block w-4" />
            )}
            <span className="text-foreground truncate max-w-[100px]">
              {col.name}
            </span>
            <span className="ml-auto text-muted-foreground text-[11px]">
              {col.type}
            </span>
          </div>
        ))}
      </div>
      <Handle type="source" position={Position.Bottom} className="!bg-transparent" />
    </div>
  );
}

export default memo(TableNode);
```

- [ ] **Step 4: Create ERDiagramCanvas.tsx**

```tsx
import { useCallback, useMemo } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  MarkerType,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import TableNode from "./TableNode";
import { computeLayout } from "./erDiagramLayout";
import type { ERDiagramData } from "./types";

const nodeTypes = { table: TableNode };

interface ERDiagramCanvasProps {
  data: ERDiagramData;
}

export default function ERDiagramCanvas({ data }: ERDiagramCanvasProps) {
  const initialNodes: Node[] = useMemo(
    () =>
      data.nodes.map((n) => ({
        id: n.id,
        type: "table",
        position: { x: 0, y: 0 },
        data: {
          label: n.name,
          columns: n.columns,
        },
      })),
    [data.nodes],
  );

  const initialEdges: Edge[] = useMemo(
    () =>
      data.edges.map((e) => ({
        id: e.id,
        source: e.source,
        target: e.target,
        type: "smoothstep",
        animated: true,
        markerEnd: { type: MarkerType.ArrowClosed },
        label: e.fkName,
        data: {
          onUpdate: e.onUpdate,
          onDelete: e.onDelete,
        },
      })),
    [data.edges],
  );

  const { nodes: layoutedNodes, edges: layoutedEdges } = useMemo(
    () => computeLayout(initialNodes, initialEdges),
    [initialNodes, initialEdges],
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(layoutedNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(layoutedEdges);

  return (
    <div className="w-full h-full">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        nodeTypes={nodeTypes}
        fitView
        attributionPosition="bottom-left"
      >
        <Background />
        <Controls />
        <MiniMap />
      </ReactFlow>
    </div>
  );
}
```

- [ ] **Step 5: Create ERDiagramView.tsx**

```tsx
import { useEffect, useState } from "react";
import { api, type SchemaForeignKey, type SchemaOverview } from "@/services/api";
import ERDiagramCanvas from "./ERDiagramCanvas";
import { buildDiagramData } from "./types";
import { useTranslation } from "react-i18next";

interface ERDiagramViewProps {
  connectionId: number;
  database?: string;
  schema?: string;
}

export default function ERDiagramView({
  connectionId,
  database,
  schema,
}: ERDiagramViewProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [diagramData, setDiagramData] = useState<ReturnType<typeof buildDiagramData> | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function fetchData() {
      try {
        setLoading(true);
        setError(null);

        const [overview, foreignKeys] = await Promise.all([
          api.metadata.getSchemaOverview(connectionId, database, schema),
          api.metadata.getSchemaForeignKeys(connectionId, database),
        ]);

        if (cancelled) return;

        if (foreignKeys.length === 0) {
          setError(t("erDiagram.noForeignKeys"));
          setLoading(false);
          return;
        }

        const data = buildDiagramData(overview, foreignKeys);
        setDiagramData(data);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchData();
    return () => { cancelled = true; };
  }, [connectionId, database, schema, t]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <span className="text-muted-foreground">{t("erDiagram.loading")}</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full">
        <span className="text-muted-foreground">{error}</span>
      </div>
    );
  }

  if (!diagramData) return null;

  return <ERDiagramCanvas data={diagramData} />;
}
```

- [ ] **Step 6: Verify frontend compiles**

Run: `bun run typecheck`
Expected: No type errors

- [ ] **Step 7: Commit**

```bash
git add src/components/business/ERDiagram/
git commit -m "feat(er): create ERDiagram components with React Flow + dagre"
```

---

## Task 5: Integration — Tab type, toolbar button, i18n

### Files
- Modify: `src/App.tsx:92-145, 1191+`
- Modify: `src/lib/i18n/locales/en.ts`
- Modify: `src/lib/i18n/locales/zh.ts`

### Steps

- [ ] **Step 1: Add "er-diagram" to TabItem type**

In `src/App.tsx`, add `"er-diagram"` to the `type` union (line 94-105):

```typescript
type:
  | "editor"
  | "table"
  | "ddl"
  | "routine"
  | "create-table"
  | "alter-table"
  | "redis-key"
  | "redis-console"
  | "redis-browser"
  | "redis-server-info"
  | "elasticsearch-index"
  | "er-diagram";
```

- [ ] **Step 2: Add ERDiagramView import**

Add at the top of `App.tsx`:

```typescript
import ERDiagramView from "@/components/business/ERDiagram/ERDiagramView";
```

- [ ] **Step 3: Add handler to open ER diagram tab**

Add a function (near other tab-opening handlers like `handleOpenTableDDL`):

```typescript
const handleOpenERDiagram = useCallback(() => {
  if (!activeConnection?.id || !activeDatabase) return;

  const tabId = `er-diagram-${activeDatabase}`;
  const existing = tabs.find((t) => t.id === tabId);
  if (existing) {
    setActiveTabId(tabId);
    return;
  }

  const newTab: TabItem = {
    id: tabId,
    type: "er-diagram",
    title: `ER - ${activeDatabase}`,
    connectionId: activeConnection.id,
    database: activeDatabase,
    schema: activeSchema,
  };
  setTabs([...tabs, newTab]);
  setActiveTabId(tabId);
}, [activeConnection, activeDatabase, activeSchema, tabs]);
```

- [ ] **Step 4: Add toolbar button**

In the toolbar area of the app (near the database selector or in a relevant toolbar section), add a button:

```tsx
<button
  onClick={handleOpenERDiagram}
  disabled={!activeConnection || !activeDatabase}
  className="px-2 py-1 text-xs border rounded hover:bg-accent disabled:opacity-50"
  title={t("erDiagram.title")}
>
  ER
</button>
```

- [ ] **Step 5: Render ERDiagramView in tab content**

In the tab content rendering area (where other tab types are rendered), add:

```tsx
{activeTab?.type === "er-diagram" && activeTab.connectionId && (
  <ERDiagramView
    connectionId={activeTab.connectionId}
    database={activeTab.database}
    schema={activeTab.schema}
  />
)}
```

- [ ] **Step 6: Add i18n keys**

In `src/lib/i18n/locales/en.ts`, add:

```typescript
erDiagram: {
  title: "ER Diagram",
  noForeignKeys: "No foreign key relationships found",
  loading: "Loading ER diagram...",
},
```

In `src/lib/i18n/locales/zh.ts`, add:

```typescript
erDiagram: {
  title: "ER 图",
  noForeignKeys: "未找到外键关系",
  loading: "加载 ER 图中...",
},
```

- [ ] **Step 7: Verify frontend compiles**

Run: `bun run typecheck`
Expected: No type errors

- [ ] **Step 8: Commit**

```bash
git add src/App.tsx src/lib/i18n/locales/en.ts src/lib/i18n/locales/zh.ts
git commit -m "feat(er): integrate ER diagram tab, toolbar button, and i18n"
```

---

## Task 6: Testing — Verify integration

### Steps

- [ ] **Step 1: Run Rust tests**

Run: `cargo test` from `src-tauri/`
Expected: All existing tests pass

- [ ] **Step 2: Run frontend lint and typecheck**

Run: `bun run lint && bun run typecheck`
Expected: No errors

- [ ] **Step 3: Test mock mode**

Run: `bun dev:mock`
- Click the "ER" button in the toolbar
- Verify the ER diagram tab opens
- Verify 3 tables appear (users, roles, orders)
- Verify edges connect them
- Verify nodes are draggable
- Verify zoom/pan works

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix(er): address integration issues"
```
