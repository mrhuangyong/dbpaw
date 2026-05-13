# 多SQL执行结果展示 - 完整改造计划

## 一、需求背景

当前架构下，多个SQL语句一起执行时，只返回最后一条语句的结果。用户希望每个SQL语句的结果都能展示出来，用Tab来切换不同的结果集。

## 二、当前架构分析

### 执行流程
```
前端: "INSERT INTO t VALUES(1); SELECT * FROM t;" (不拆分，整体发送)
         ↓
后端: split_sql_statements() 按分号分割
         ↓
     逐个执行，只返回最后一条结果
```

### 核心问题
- `QueryResult` 结构只支持单一结果集
- `DatabaseDriver` trait 返回单个 `QueryResult`
- 所有8个驱动都遵循"执行N-1条，只返回最后一条"的模式

## 三、目标架构

```
┌─────────────────────────────────────────────────────────────────┐
│  目标架构 (多结果集)                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐          │
│  │  SQL Editor │───▶│  Backend    │───▶│  结果集 1   │          │
│  │             │    │  (逐条执行)  │    │  结果集 2   │          │
│  │   Tab 切换  │    │             │    │  结果集 N   │          │
│  └─────────────┘    └─────────────┘    └─────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

## 四、分阶段实施计划

### Phase 1: 后端模型扩展 ✅ 已完成

**目标**: 扩展数据结构，不改变现有行为

**改动文件**:
- `src-tauri/src/models/mod.rs`
  - 新增 `SingleResultSet` 结构体
  - 扩展 `QueryResult` 添加 `result_sets: Option<Vec<SingleResultSet>>` 字段
- `src-tauri/src/db/drivers/*.rs` (7个驱动文件)
  - 所有 `QueryResult` 初始化添加 `result_sets: None`

**新增结构体**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleResultSet {
    pub data: Vec<serde_json::Value>,
    pub row_count: i64,
    pub columns: Vec<QueryColumn>,
    pub index: u32,
    pub statement: String,
}
```

**扩展 QueryResult**:
```rust
pub struct QueryResult {
    // ... 现有字段保持不变
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_sets: Option<Vec<SingleResultSet>>,
}
```

**验证**: `cargo check` 通过

---

### Phase 2: 前端类型同步

**目标**: TypeScript 接口与后端保持一致

**改动文件**:
- `src/services/api.ts`

**新增接口**:
```typescript
export interface SingleResultSet {
  data: any[];
  rowCount: number;
  columns: QueryColumn[];
  index: number;
  statement: string;
}
```

**扩展接口**:
```typescript
export interface QueryResult {
  // ... 现有字段
  resultSets?: SingleResultSet[];
}
```

**验证**: `bun run typecheck` 通过

---

### Phase 3: 驱动改造 - 逐条执行并收集结果

**目标**: 所有驱动执行多SQL时返回完整结果集

**改造策略**:
- 单条SQL: 行为不变，`result_sets` 为 `None`
- 多条SQL: 逐条执行，收集所有结果到 `result_sets`

| 数据库 | 文件 | 改造难度 | 说明 |
|--------|------|---------|------|
| **MySQL/MariaDB** | `src-tauri/src/db/drivers/mysql.rs` | 中 | 逐条执行，SELECT 类型 fetch_all，DML 类型返回 rows_affected |
| **PostgreSQL** | `src-tauri/src/db/drivers/postgres.rs` | 中 | 同 MySQL |
| **SQLite** | `src-tauri/src/db/drivers/sqlite.rs` | 中 | 同 MySQL |
| **DuckDB** | `src-tauri/src/db/drivers/duckdb.rs` | 中 | 同 MySQL |
| **MSSQL** | `src-tauri/src/db/drivers/mssql.rs` | 低 | tiberius 原生支持多结果集 |
| **ClickHouse** | `src-tauri/src/db/drivers/clickhouse.rs` | 高 | HTTP 协议限制，需逐条请求 |
| **Oracle** | `src-tauri/src/db/drivers/oracle.rs` | 中 | 同 MySQL |

**核心改动逻辑** (以 MySQL 为例):
```rust
async fn execute_query(&self, sql: String) -> Result<QueryResult, String> {
    let start = std::time::Instant::now();
    let statements = super::split_sql_statements(&sql);
    
    if statements.is_empty() {
        return Err("[QUERY_ERROR] Empty SQL statement".to_string());
    }
    
    // 单条语句：保持原有行为
    if statements.len() == 1 {
        // ... 现有逻辑不变
        return Ok(QueryResult {
            // ... 现有字段
            result_sets: None,  // 单条不填充
        });
    }
    
    // 多条语句：逐条执行并收集结果
    let mut result_sets = Vec::new();
    let mut last_error: Option<String> = None;
    
    for (idx, statement) in statements.iter().enumerate() {
        match self.execute_single_query(statement).await {
            Ok((columns, data, row_count)) => {
                result_sets.push(SingleResultSet {
                    data,
                    row_count,
                    columns,
                    index: idx as u32,
                    statement: statement.clone(),
                });
            }
            Err(e) => {
                last_error = Some(e);
                break;  // 短路中断
            }
        }
    }
    
    let duration = start.elapsed();
    
    if let Some(err) = last_error {
        // 部分成功，返回已有结果 + 错误信息
        return Ok(QueryResult {
            data: vec![],
            row_count: 0,
            columns: vec![],
            time_taken_ms: duration.as_millis() as i64,
            success: false,
            error: Some(err),
            result_sets: Some(result_sets),  // 包含已成功的结果
        });
    }
    
    // 全部成功
    Ok(QueryResult {
        data: vec![],  // 多条时不填充顶层 data
        row_count: 0,
        columns: vec![],
        time_taken_ms: duration.as_millis() as i64,
        success: true,
        error: None,
        result_sets: Some(result_sets),
    })
}
```

**验证**:
- `cargo check` 通过
- 现有单条SQL测试不受影响
- 新增多结果集集成测试

---

### Phase 4: 前端状态管理改造

**目标**: 支持多结果集状态

**改动文件**:
- `src/lib/queryExecutionState.ts`

**扩展类型**:
```typescript
// 新增单结果集状态
export type SingleResultState = {
  data: unknown[];
  columns: string[];
  rowCount: number;
  statement: string;
  index: number;
};

// 扩展查询结果状态
export type QueryResultsState = {
  // 保持向后兼容
  data: unknown[];
  columns: string[];
  executionTime: string;
  error?: string;
  // 新增：多结果集
  resultSets?: SingleResultState[];
  // 新增：当前选中的结果集索引
  activeResultSetIndex?: number;
};
```

**验证**: `bun run typecheck` 通过

---

### Phase 5: App.tsx 逻辑改造

**目标**: 处理多结果集响应

**改动文件**:
- `src/App.tsx`

**TabItem 接口扩展**:
```typescript
interface TabItem {
  // ... 现有字段
  queryResults?: {
    data: any[];
    columns: string[];
    executionTime: string;
    error?: string;
    resultSets?: SingleResultState[];  // 新增
    activeResultSetIndex?: number;      // 新增
  } | null;
}
```

**handleExecuteQuery 改造**:
```typescript
const handleExecuteQuery = async (tabId: string, sql: string) => {
  // ... 现有逻辑
  
  const result = await api.query.execute(/* ... */);
  
  // 处理多结果集
  const resultSets = result.resultSets?.map(rs => ({
    data: rs.data,
    columns: rs.columns.map(c => c.name),
    rowCount: rs.rowCount,
    statement: rs.statement,
    index: rs.index,
  }));
  
  setTabs(prev => prev.map(t =>
    applyQueryCompletionToTab(t, tabId, queryId, {
      data: result.data || [],
      columns: (result.columns || []).map(c => c.name),
      executionTime: `${execMs}ms`,
      resultSets,  // 传递多结果集
      activeResultSetIndex: resultSets?.length ? 0 : undefined,
    })
  ));
};
```

**验证**: `bun run typecheck` 通过

---

### Phase 6: SqlEditor UI 改造

**目标**: 添加结果集切换 Tab UI

**改动文件**:
- `src/components/business/Editor/SqlEditor.tsx`

**UI 设计**:
```
┌─────────────────────────────────────────────────────────────┐
│  SQL Editor                                                 │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ SELECT * FROM users;                                    ││
│  │ SELECT * FROM orders;                                   ││
│  │ SELECT * FROM products;                                 ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │ ▶ Execute                                                ││
│  └─────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│  ┌─────────┐ ┌─────────┐ ┌─────────┐                       │
│  │ Result 1│ │ Result 2│ │ Result 3│  ← 结果集切换 Tab      │
│  │ (3 rows)│ │ (5 rows)│ │ (2 rows)│                       │
│  └─────────┘ └─────────┘ └─────────┘                       │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  TableView (当前选中结果集的数据)                          ││
│  │  ┌────┬──────────┬───────────┐                          ││
│  │  │ id │ name     │ email     │                          ││
│  │  ├────┼──────────┼───────────┤                          ││
│  │  │ 1  │ Alice    │ a@test.com│                          ││
│  │  │ 2  │ Bob      │ b@test.com│                          ││
│  │  └────┴──────────┴───────────┘                          ││
│  └─────────────────────────────────────────────────────────┘│
│  ✓ Success - 3 results (100ms)                              │
└─────────────────────────────────────────────────────────────┘
```

**关键实现**:
```typescript
// SqlEditor.tsx 新增状态
const [activeResultSetIndex, setActiveResultSetIndex] = useState(0);

// 判断是否有多结果集
const hasMultipleResults = queryResults?.resultSets && queryResults.resultSets.length > 1;

// 当前显示的结果集
const currentResultSet = hasMultipleResults 
  ? queryResults.resultSets[activeResultSetIndex]
  : queryResults;

// 结果集切换 Tab
{hasMultipleResults && (
  <div className="flex border-b">
    {queryResults.resultSets.map((rs, idx) => (
      <button
        key={idx}
        className={`px-3 py-1.5 text-sm ${
          idx === activeResultSetIndex 
            ? 'border-b-2 border-primary' 
            : 'text-muted-foreground'
        }`}
        onClick={() => setActiveResultSetIndex(idx)}
      >
        Result {idx + 1} ({rs.rowCount} rows)
      </button>
    ))}
  </div>
)}

// 显示当前结果集
<TableView 
  data={currentResultSet?.data || []} 
  columns={currentResultSet?.columns || []} 
/>
```

**验证**:
- `bun run typecheck` 通过
- `bun dev:mock` 手动测试 UI

---

### Phase 7: Mock 系统更新

**目标**: 支持多结果集 mock 数据

**改动文件**:
- `src/services/mocks.ts`

---

### Phase 8: 测试更新

**目标**: 确保改造质量

**测试类型**:
- `src/lib/queryExecutionState.unit.test.ts` - 新增多结果集测试用例
- `src-tauri/src/db/drivers/mod.rs` - 更新 `split_sql_statements` 测试
- `src-tauri/tests/*_command_integration.rs` (16个文件) - 适配新返回类型

---

## 五、风险与缓解措施

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| **ClickHouse HTTP 协议限制** | 高 | 多语句场景逐条请求，或在 UI 标注"仅显示最后结果" |
| **内存压力** | 中 | 限制单结果集行数 (如 10000 行)；可选懒加载 |
| **执行时间累积** | 中 | 显示总耗时和各结果集耗时；支持取消操作 |
| **向后兼容** | 低 | `resultSets` 为 `Optional`，单条 SQL 行为不变 |
| **测试覆盖** | 中 | 分阶段实施，每阶段独立可测试 |

---

## 六、验收标准

1. **功能验收**:
   - [ ] 单条 SQL 执行行为不变
   - [ ] 多条 SQL 执行返回所有结果集
   - [ ] 结果集 Tab 切换正常
   - [ ] 部分失败时显示已有结果 + 错误信息
   - [ ] 所有 8 个数据库驱动支持

2. **质量验收**:
   - [ ] `bun run typecheck` 通过
   - [ ] `bun run lint` 通过
   - [ ] `cargo check` 通过
   - [ ] `bun run test:unit` 通过
   - [ ] `bun run test:rust:unit` 通过
   - [ ] 现有集成测试不受影响

3. **UI/UX 验收**:
   - [ ] 结果集 Tab 样式与现有 UI 一致
   - [ ] Tab 显示结果集索引和行数
   - [ ] 错误信息清晰可见
   - [ ] 响应式布局正常

---

## 七、工时估算

| Phase | 内容 | 预估工时 | 状态 |
|-------|------|---------|------|
| Phase 1 | 后端模型扩展 | 0.5h | ✅ 已完成 |
| Phase 2 | 前端类型同步 | 0.5h | ✅ 已完成 |
| Phase 3 | 驱动改造 (8个) | 4h | 待开始 |
| Phase 4 | 前端状态管理 | 1h | 待开始 |
| Phase 5 | App.tsx 逻辑改造 | 1h | 待开始 |
| Phase 6 | SqlEditor UI 改造 | 2h | 待开始 |
| Phase 7 | Mock 系统更新 | 0.5h | 待开始 |
| Phase 8 | 测试更新 | 2h | 待开始 |
| **总计** | | **11.5h** | |

---

## 八、相关文件清单

### 后端 Rust
- `src-tauri/src/models/mod.rs` - 数据结构定义
- `src-tauri/src/db/drivers/mod.rs` - DatabaseDriver trait
- `src-tauri/src/db/drivers/mysql.rs` - MySQL 驱动
- `src-tauri/src/db/drivers/postgres.rs` - PostgreSQL 驱动
- `src-tauri/src/db/drivers/sqlite.rs` - SQLite 驱动
- `src-tauri/src/db/drivers/duckdb.rs` - DuckDB 驱动
- `src-tauri/src/db/drivers/mssql.rs` - MSSQL 驱动
- `src-tauri/src/db/drivers/clickhouse.rs` - ClickHouse 驱动
- `src-tauri/src/db/drivers/oracle.rs` - Oracle 驱动
- `src-tauri/src/commands/query.rs` - Tauri 命令层

### 前端 TypeScript
- `src/services/api.ts` - API 接口定义
- `src/App.tsx` - 主应用组件
- `src/lib/queryExecutionState.ts` - 查询状态管理
- `src/components/business/Editor/SqlEditor.tsx` - SQL 编辑器组件
- `src/services/mocks.ts` - Mock 数据

### 测试文件
- `src/lib/queryExecutionState.unit.test.ts`
- `src-tauri/tests/*_command_integration.rs` (16个文件)

---

## 九、进度记录

### 2026-05-13
- ✅ Phase 1 完成：后端模型扩展
  - 新增 `SingleResultSet` 结构体
  - 扩展 `QueryResult` 添加 `result_sets` 字段
  - 修复所有驱动文件的 `QueryResult` 初始化
  - `cargo check` 验证通过
- ✅ Phase 2 完成：前端类型同步
  - 新增 `SingleResultSet` 接口
  - 扩展 `QueryResult` 添加 `resultSets` 字段
  - `bun run typecheck` 验证通过
