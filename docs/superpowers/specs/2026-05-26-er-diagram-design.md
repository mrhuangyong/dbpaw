# ER Diagram Design

> Visualize table relationships as an interactive ER diagram for the entire database/schema.

## Goal

Add an ER diagram feature to DbPaw that displays all tables in the current database/schema with their foreign key relationships as a draggable, zoomable graph.

## Scope

- **In scope**: All tables in current database/schema
- **Out of scope**: Table subset selection, editing relationships, creating/modifying FKs
- **Supported databases**: PostgreSQL, MySQL, MSSQL, Oracle, SQLite (databases with FK support)

## Trigger

Top toolbar button opens an ER diagram tab for the current database.

## Data Flow

```
User clicks toolbar button
  → api.metadata.getSchemaOverview()  (tables + columns)
  → api.metadata.getSchemaForeignKeys()  (all FK relationships)
  → Frontend assembles nodes (tables) + edges (FK connections)
  → dagre auto-layout
  → React Flow renders interactive diagram
```

## Backend Changes

### New Type: `SchemaForeignKey`

File: `src-tauri/src/models/mod.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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

### New Command: `get_schema_foreign_keys`

File: `src-tauri/src/commands/metadata.rs`

```rust
#[tauri::command]
pub async fn get_schema_foreign_keys(
    state: State<'_, AppState>,
    connection_id: i64,
    database: Option<String>,
) -> Result<Vec<SchemaForeignKey>, String>
```

### Driver Trait Extension

File: `src-tauri/src/db/drivers/mod.rs`

Add default method to `DatabaseDriver` trait:

```rust
async fn get_schema_foreign_keys(
    &self,
    database: Option<&str>,
) -> Result<Vec<SchemaForeignKey>, String> {
    Ok(vec![])  // default: no FK support
}
```

Override in: Postgres, MySQL, MSSQL, Oracle, SQLite

### Driver Implementations

| Driver   | Query Source |
|----------|-------------|
| Postgres | `pg_constraint` + `pg_class` + `pg_attribute` |
| MySQL    | `INFORMATION_SCHEMA.KEY_COLUMN_USAGE` + `REFERENTIAL_CONSTRAINTS` |
| MSSQL    | `sys.foreign_keys` + `sys.foreign_key_columns` |
| Oracle   | `USER_CONSTRAINTS` + `USER_CONS_COLUMNS` |
| SQLite   | `pragma_foreign_key_list` per table |

## Frontend Changes

### New Component: `src/components/business/ERDiagram/`

```
ERDiagram/
├── ERDiagramView.tsx      # Main container, data fetching, state
├── ERDiagramCanvas.tsx     # React Flow canvas, node/edge rendering
├── TableNode.tsx           # Custom node: table name + PK/FK columns
├── erDiagramLayout.ts      # dagre layout computation
└── types.ts                # ER diagram type definitions
```

### TableNode Visual Design

```
┌──────────────────────┐
│  📋 users            │  ← table name (bold header)
├──────────────────────┤
│  PK id          int  │  ← PK column (yellow badge)
│  FK role_id     int  │  ← FK column (blue badge)
│  FK dept_id     int  │
└──────────────────────┘
```

- Only PK and FK columns shown (compact mode)
- Column type displayed on the right
- PK columns highlighted with yellow badge
- FK columns highlighted with blue badge

### Canvas Capabilities

- Drag nodes to reposition
- Zoom (scroll wheel) + pan (drag empty area)
- Edges show FK relationships with arrows pointing to referenced table
- Hover edge to show FK name and ON DELETE/UPDATE actions
- MiniMap for navigation on large schemas

### Tab Integration

- New tab type: `"er-diagram"`
- Tab title: "ER Diagram - {database}"

### API Layer

File: `src/services/api.ts`

```typescript
getSchemaForeignKeys: (connectionId: number, database?: string) =>
  invoke('get_schema_foreign_keys', { connectionId, database }),
```

File: `src/services/mocks.ts` — add mock implementation

## Layout Algorithm

**dagre configuration:**

```typescript
const g = new dagre.graphlib.Graph();
g.setDefaultEdgeLabel(() => ({}));
g.setGraph({ rankdir: 'TB', nodesep: 80, ranksep: 100 });

// Node size: dynamic based on column count
nodes.forEach((node) => {
  const height = 40 + node.data.columns.length * 28;
  g.setNode(node.id, { width: 220, height });
});

edges.forEach((edge) => {
  g.setEdge(edge.source, edge.target);
});

dagre.layout(g);
```

- **Direction**: Top-to-bottom (`TB`)
- **Node spacing**: 80px horizontal, 100px vertical
- **Node width**: Fixed 220px
- **Node height**: Dynamic (header 40px + 28px per column)
- **Isolated tables**: No FK relationships placed in separate area (right side)
- **Default view**: `fitView` to canvas
- **Navigation**: MiniMap component

## Dependencies

| Package | Purpose | Size |
|---------|---------|------|
| `@xyflow/react` | React Flow v12 - interactive graph canvas | ~150KB |
| `dagre` | Automatic hierarchical layout | ~50KB |
| `@types/dagre` | TypeScript types for dagre | dev only |

## i18n

New keys in `en.ts` and `zh.ts`:
- `erDiagram.title` — "ER Diagram"
- `erDiagram.noForeignKeys` — "No foreign key relationships found"
- `erDiagram.loading` — "Loading ER diagram..."
