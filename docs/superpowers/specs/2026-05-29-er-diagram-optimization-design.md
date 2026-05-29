# ER Diagram 优化设计

**日期：** 2026-05-29  
**状态：** 已批准

## 背景

ER Diagram 功能需要以下优化：
1. ER 按钮位置从标题栏移到表格工具栏
2. 侧边栏数据库右键菜单添加 ER 功能
3. MySQL 表节点右键菜单与数据库右键菜单保持一致

## 设计

### 1. ER 按钮位置调整

**目标：** 将 ER 按钮从窗口标题栏移到表格视图工具栏（与"新建查询"、"DDL"按钮同行）

**改动点：**

1. **移除标题栏 ER 按钮** (`src/App.tsx`)
   - 删除 `renderWindowActions()` 中的 ER 按钮代码（约第 400-409 行）
   - 保留 `handleOpenERDiagram` 回调函数

2. **在表格工具栏添加 ER 按钮** (`src/components/business/DataGrid/TableView.tsx`)
   - 在 DDL 按钮旁边添加 ER 按钮
   - 按钮样式：`variant="ghost" size="sm" className="h-6 gap-1 px-2 hover:bg-muted/60"`
   - 图标：使用 `Table` icon（与现有 ER Diagram tab 标题保持一致）
   - 点击后调用 `onOpenERDiagram` 回调
   - 始终显示，不需要条件判断

**数据流：**
- `TableView` 组件新增 prop: `onOpenERDiagram?: (ctx: TableContext) => void`
- `App.tsx` 传递 `handleOpenERDiagram` 给 `TableView`

### 2. 数据库右键菜单添加 ER

**目标：** 在侧边栏数据库节点的右键菜单中添加 ER Diagram 选项

**改动点：**

1. **扩展 TreeCallbacks 类型** (`src/lib/tree-adapters/types.tsx`)
   - 添加回调：`onOpenERDiagram?: (ctx: DatabaseContext) => void`

2. **添加数据库右键菜单函数** (`src/lib/tree-adapters/sql-adapter.tsx`)
   - 新增 `getSqlDatabaseContextMenuItems()` 函数
   - 菜单项：
     - 新建查询 (FileCode icon)
     - ER Diagram (Table icon)

3. **在 ConnectionList 中传递回调** (`src/components/business/Sidebar/ConnectionList.tsx`)
   - 在构建 `configWithEnhancedCallbacks` 时添加 `onOpenERDiagram` 回调
   - 回调逻辑：打开一个新的 ER Diagram tab

**菜单项顺序：**
```
新建查询
ER Diagram
```

### 3. 表节点右键菜单统一

**目标：** 使所有 SQL 驱动的表节点右键菜单与数据库右键菜单保持一致

**改动点：**

1. **扩展表右键菜单** (`src/lib/tree-adapters/sql-adapter.tsx`)
   - 修改 `getSqlLeafContextMenuItems()` 函数
   - 添加 `onRefresh` 和 `onOpenERDiagram` 回调参数

2. **菜单项（按顺序）：**
   - 新建查询 (FileCode icon)
   - 刷新 (RefreshCw icon)
   - ER Diagram (Table icon)
   - 导出表 (Download icon)
   - 修改表结构 (Edit3 icon)

3. **回调签名：**
   ```typescript
   callbacks: {
     onCreateQuery?: (ctx: DatabaseContext) => void;
     onRefresh?: (ctx: DatabaseContext) => void;
     onOpenERDiagram?: (ctx: DatabaseContext) => void;
     onExportTable?: (ctx: LeafContext) => void;
     onAlterTable?: (ctx: LeafContext) => void;
   }
   ```

**效果：**
- 所有使用 `createSqlTreeConfig()` 的驱动（MySQL、PostgreSQL、SQLite、MariaDB、TiDB 等）都会有统一的右键菜单
- 数据库节点和表节点的菜单项保持一致（除了"修改表结构"只在表节点显示）

## 需要修改的文件

1. `src/App.tsx` - 移除标题栏 ER 按钮
2. `src/components/business/DataGrid/TableView.tsx` - 添加表格工具栏 ER 按钮
3. `src/lib/tree-adapters/types.tsx` - 添加 `onOpenERDiagram` 回调
4. `src/lib/tree-adapters/sql-adapter.tsx` - 添加右键菜单函数
5. `src/components/business/Sidebar/ConnectionList.tsx` - 传递回调

## 测试计划

1. 验证 ER 按钮在表格工具栏正确显示
2. 验证点击 ER 按钮能打开 ER Diagram tab
3. 验证数据库右键菜单包含"新建查询"和"ER Diagram"
4. 验证表节点右键菜单包含所有 5 个选项
5. 验证所有 SQL 驱动都有统一的右键菜单
6. 验证从右键菜单打开 ER Diagram 功能正常