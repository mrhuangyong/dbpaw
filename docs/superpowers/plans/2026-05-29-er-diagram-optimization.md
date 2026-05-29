# ER Diagram 优化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 优化 ER Diagram 功能：移动按钮位置、添加右键菜单、统一菜单项

**Architecture:** 修改 5 个文件，将 ER 按钮从标题栏移到表格工具栏，为 SQL 驱动添加数据库和表节点的右键菜单

**Tech Stack:** React, TypeScript, Lucide icons

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `src/lib/tree-adapters/types.tsx` | 添加 `onOpenERDiagram` 回调类型 |
| `src/lib/tree-adapters/sql-adapter.tsx` | 添加数据库和表右键菜单函数 |
| `src/App.tsx` | 移除标题栏 ER 按钮，传递回调给 TableView |
| `src/components/business/DataGrid/TableView.tsx` | 添加表格工具栏 ER 按钮 |
| `src/components/business/Sidebar/ConnectionList.tsx` | 传递 `onOpenERDiagram` 回调 |

---

### Task 1: 扩展 TreeCallbacks 类型

**Files:**
- Modify: `src/lib/tree-adapters/types.tsx:36-62`

- [ ] **Step 1: 添加 onOpenERDiagram 回调**

在 `TreeCallbacks` 接口中添加新的回调：

```typescript
export interface TreeCallbacks {
  // SQL 类通用
  onTableSelect?: (ctx: LeafContext) => void;
  onCreateQuery?: (ctx: DatabaseContext) => void;
  onOpenERDiagram?: (ctx: DatabaseContext) => void;  // 新增
  onExportTable?: (ctx: LeafContext) => void;
  onAlterTable?: (ctx: LeafContext) => void;

  // ... 其他回调保持不变
}
```

- [ ] **Step 2: 验证类型检查**

Run: `npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src/lib/tree-adapters/types.tsx
git commit -m "feat: add onOpenERDiagram callback to TreeCallbacks"
```

---

### Task 2: 添加数据库右键菜单函数

**Files:**
- Modify: `src/lib/tree-adapters/sql-adapter.tsx`

- [ ] **Step 1: 添加 getSqlDatabaseContextMenuItems 函数**

在文件末尾添加新函数：

```typescript
export function getSqlDatabaseContextMenuItems(
  ctx: DatabaseContext,
  callbacks: {
    onCreateQuery?: (ctx: DatabaseContext) => void;
    onOpenERDiagram?: (ctx: DatabaseContext) => void;
  },
): TreeMenuItem[] {
  const items: TreeMenuItem[] = [];

  if (callbacks.onCreateQuery) {
    items.push({
      key: "new-query",
      label: "New Query",
      icon: <FileCode className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onCreateQuery!(ctx),
    });
  }

  if (callbacks.onOpenERDiagram) {
    items.push({
      key: "er-diagram",
      label: "ER Diagram",
      icon: <Table className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onOpenERDiagram!(ctx),
    });
  }

  return items;
}
```

- [ ] **Step 2: 验证类型检查**

Run: `npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src/lib/tree-adapters/sql-adapter.tsx
git commit -m "feat: add getSqlDatabaseContextMenuItems function"
```

---

### Task 3: 扩展表右键菜单函数

**Files:**
- Modify: `src/lib/tree-adapters/sql-adapter.tsx:23-68`

- [ ] **Step 1: 修改 getSqlLeafContextMenuItems 函数签名**

```typescript
export function getSqlLeafContextMenuItems(
  ctx: LeafContext,
  callbacks: {
    onCreateQuery?: (ctx: DatabaseContext) => void;
    onRefresh?: (ctx: DatabaseContext) => void;
    onOpenERDiagram?: (ctx: DatabaseContext) => void;
    onExportTable?: (ctx: LeafContext) => void;
    onAlterTable?: (ctx: LeafContext) => void;
  },
): TreeMenuItem[] {
```

- [ ] **Step 2: 添加刷新和 ER Diagram 菜单项**

在现有菜单项之前添加：

```typescript
export function getSqlLeafContextMenuItems(
  ctx: LeafContext,
  callbacks: {
    onCreateQuery?: (ctx: DatabaseContext) => void;
    onRefresh?: (ctx: DatabaseContext) => void;
    onOpenERDiagram?: (ctx: DatabaseContext) => void;
    onExportTable?: (ctx: LeafContext) => void;
    onAlterTable?: (ctx: LeafContext) => void;
  },
): TreeMenuItem[] {
  const items: TreeMenuItem[] = [];

  if (callbacks.onCreateQuery) {
    items.push({
      key: "new-query",
      label: "New Query",
      icon: <FileCode className="mr-2 h-4 w-4" />,
      onClick: () =>
        callbacks.onCreateQuery!({
          connectionId: ctx.connectionId,
          connectionName: ctx.connectionName,
          connectionType: ctx.connectionType,
          driverKind: ctx.driverKind,
          databaseName: ctx.databaseName,
        }),
    });
  }

  if (callbacks.onRefresh) {
    items.push({
      key: "refresh",
      label: "Refresh",
      icon: <RefreshCw className="mr-2 h-4 w-4" />,
      onClick: () =>
        callbacks.onRefresh!({
          connectionId: ctx.connectionId,
          connectionName: ctx.connectionName,
          connectionType: ctx.connectionType,
          driverKind: ctx.driverKind,
          databaseName: ctx.databaseName,
        }),
    });
  }

  if (callbacks.onOpenERDiagram) {
    items.push({
      key: "er-diagram",
      label: "ER Diagram",
      icon: <Table className="mr-2 h-4 w-4" />,
      onClick: () =>
        callbacks.onOpenERDiagram!({
          connectionId: ctx.connectionId,
          connectionName: ctx.connectionName,
          connectionType: ctx.connectionType,
          driverKind: ctx.driverKind,
          databaseName: ctx.databaseName,
        }),
    });
  }

  if (callbacks.onExportTable) {
    items.push({
      key: "export-table",
      label: "Export Table",
      icon: <Download className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onExportTable!(ctx),
    });
  }

  if (callbacks.onAlterTable) {
    items.push({
      key: "alter-table",
      label: "Alter Table",
      icon: <Table className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onAlterTable!(ctx),
    });
  }

  return items;
}
```

- [ ] **Step 3: 添加 RefreshCw 导入**

在文件顶部的导入中添加 `RefreshCw`：

```typescript
import { Table, Database, FileCode, Download, RefreshCw } from "lucide-react";
```

- [ ] **Step 4: 验证类型检查**

Run: `npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 5: Commit**

```bash
git add src/lib/tree-adapters/sql-adapter.tsx
git commit -m "feat: extend getSqlLeafContextMenuItems with refresh and ER diagram"
```

---

### Task 4: 移除标题栏 ER 按钮

**Files:**
- Modify: `src/App.tsx:400-409`

- [ ] **Step 1: 删除 renderWindowActions 中的 ER 按钮**

删除以下代码：

```typescript
      <Button
        variant="ghost"
        size="sm"
        className="h-7 px-2 text-xs"
        onClick={handleOpenERDiagram}
        disabled={!activeTabItem?.connectionId || !activeTabItem?.database}
        title={t("erDiagram.title")}
      >
        ER
      </Button>
```

- [ ] **Step 2: 验证类型检查**

Run: `npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat: remove ER button from title bar"
```

---

### Task 5: 添加表格工具栏 ER 按钮

**Files:**
- Modify: `src/components/business/DataGrid/TableView.tsx:1900-1915`

- [ ] **Step 1: 添加 onOpenERDiagram prop**

在 `TableViewProps` 接口中添加：

```typescript
onOpenERDiagram?: (ctx: {
  connectionId: string;
  database: string;
  schema: string;
  table: string;
  driver: string;
}) => void;
```

- [ ] **Step 2: 在组件中解构 prop**

```typescript
const {
  // ... 其他 props
  onOpenERDiagram,
} = props;
```

- [ ] **Step 3: 添加 ER 按钮**

在 DDL 按钮之后添加：

```typescript
<Button
  variant="ghost"
  size="sm"
  className="h-6 gap-1 px-2 hover:bg-muted/60"
  onClick={() => {
    if (tableContext && onOpenERDiagram) {
      onOpenERDiagram(tableContext);
    }
  }}
  title="Open ER Diagram"
>
  <Table className="w-3.5 h-3.5" />
  <span className="text-xs font-medium leading-none">
    ER
  </span>
</Button>
```

- [ ] **Step 4: 添加 Table 导入**

在文件顶部的导入中添加 `Table`：

```typescript
import { Table } from "lucide-react";
```

- [ ] **Step 5: 验证类型检查**

Run: `npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 6: Commit**

```bash
git add src/components/business/DataGrid/TableView.tsx
git commit -m "feat: add ER button to table toolbar"
```

---

### Task 6: 传递回调到 ConnectionList

**Files:**
- Modify: `src/components/business/Sidebar/ConnectionList.tsx:1783-1810`

- [ ] **Step 1: 在 renderDatabaseContextMenu 中添加 onOpenERDiagram**

找到 `renderDatabaseContextMenu` 的实现，添加 ER Diagram 回调：

```typescript
renderDatabaseContextMenu:
  configWithEnhancedCallbacks.getDatabaseContextMenuItems
    ? (databaseName) => {
        const ctx = {
          ...buildContext(),
          databaseName,
        };
        const items =
          configWithEnhancedCallbacks.getDatabaseContextMenuItems!(ctx);
        return (
          <>
            {items.map((item) => (
              <button
                key={item.key}
                className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-accent"
                onClick={() => {
                  item.onClick();
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                {item.icon}
                {item.label}
              </button>
            ))}
          </>
        );
      }
    : undefined,
```

- [ ] **Step 2: 传递 onOpenERDiagram 回调**

在构建 `configWithEnhancedCallbacks` 时添加回调：

```typescript
const configWithEnhancedCallbacks = useMemo(() => {
  const baseConfig = getTreeConfig(connection.type, {
    ...callbacks,
    onOpenERDiagram: (ctx) => {
      handleOpenERDiagram(ctx.connectionId, ctx.databaseName);
    },
  });
  // ... 其余逻辑
}, [/* dependencies */]);
```

- [ ] **Step 3: 添加 handleOpenERDiagram 函数**

在 ConnectionList 组件中添加：

```typescript
const handleOpenERDiagram = useCallback((connectionId: string, database: string) => {
  // 触发打开 ER Diagram 的事件
  // 具体实现取决于 App.tsx 中的逻辑
}, []);
```

- [ ] **Step 4: 验证类型检查**

Run: `npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 5: Commit**

```bash
git add src/components/business/Sidebar/ConnectionList.tsx
git commit -m "feat: pass onOpenERDiagram callback to ConnectionList"
```

---

### Task 7: 传递回调到 TableView

**Files:**
- Modify: `src/App.tsx:2205-2235`

- [ ] **Step 1: 在 TableView 组件中传递 onOpenERDiagram**

找到 `TableView` 组件的使用位置，添加回调：

```typescript
<TableView
  // ... 其他 props
  onOpenDDL={handleOpenTableDDL}
  onOpenERDiagram={(ctx) => {
    handleOpenERDiagram();
  }}
  // ... 其他 props
/>
```

- [ ] **Step 2: 验证类型检查**

Run: `npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: 验证构建**

Run: `npm run build`
Expected: 构建成功

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx
git commit -m "feat: pass onOpenERDiagram callback to TableView"
```

---

### Task 8: 集成测试

- [ ] **Step 1: 启动开发服务器**

Run: `npm run dev`

- [ ] **Step 2: 测试 ER 按钮位置**

1. 打开一个数据库表
2. 验证 ER 按钮在表格工具栏显示（与新建查询、DDL 同行）
3. 点击 ER 按钮，验证能打开 ER Diagram tab

- [ ] **Step 3: 测试数据库右键菜单**

1. 在侧边栏右键点击数据库节点
2. 验证菜单包含"新建查询"和"ER Diagram"
3. 点击"ER Diagram"，验证能打开 ER Diagram tab

- [ ] **Step 4: 测试表右键菜单**

1. 在侧边栏右键点击表节点
2. 验证菜单包含：新建查询、刷新、ER Diagram、导出表、修改表结构
3. 点击"ER Diagram"，验证能打开 ER Diagram tab

- [ ] **Step 5: 测试所有 SQL 驱动**

1. 连接 MySQL 数据库，验证右键菜单正常
2. 连接 PostgreSQL 数据库，验证右键菜单正常
3. 连接 SQLite 数据库，验证右键菜单正常

- [ ] **Step 6: 最终 Commit**

```bash
git add -A
git commit -m "feat: complete ER diagram optimization"
```