# Cassandra 功能差距清单

逐项修复，完成后勾选 `[x]`。

---

## 严重（功能损坏）

### [ ] 1. `create_database_by_id` 缺少 Cassandra 路由

**现象**: 前端 `driver-registry.tsx` 标记 `supportsCreateDatabase: true`，但后端 `commands/connection.rs` 的 `create_database_by_id` 没有 `"cassandra"` match arm，点击"创建数据库"会返回 `[UNSUPPORTED] Driver cassandra not supported`。

**修复方案**:
- 在 `src-tauri/src/commands/connection.rs` 的 `create_database_by_id` 中添加 `build_cassandra_create_database_sql()` 函数
- 生成 `CREATE KEYSPACE IF NOT EXISTS <name> WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}` 语句
- 需要考虑用户是否需要自定义 replication strategy（SimpleStrategy vs NetworkTopologyStrategy）

**涉及文件**:
- `src-tauri/src/commands/connection.rs`

---

## 高优先级（UI 缺失）

### [ ] 2. `TableMetadataView` 未渲染 `cassandraExtra`

**现象**: 后端 `get_table_metadata` 已返回 `cassandra_extra`（partition key、clustering columns、compaction strategy、bloom filter、gc_grace_seconds、default_time_to_live），前端 `TableMetadata` 接口也已定义 `CassandraTableExtra`，但 `TableMetadataView.tsx` 组件没有渲染它。

**修复方案**:
- 在 `src/components/business/Metadata/TableMetadataView.tsx` 中添加类似 `clickhouseExtra` 的渲染块
- 展示: Partition Key、Clustering Columns、Compaction Strategy、Bloom Filter FP Chance、GC Grace Seconds、Default TTL
- i18n 翻译已就绪（`tableMetadata.cassandra.*`）

**涉及文件**:
- `src/components/business/Metadata/TableMetadataView.tsx`

---

### [ ] 3. 无自定义树适配器

**现象**: Cassandra 复用 `createSqlTreeConfig()`，缺少 Cassandra 专属功能:
- 右键菜单无 "Truncate Table"、"Drop Keyspace"、"Copy CQL"
- 无 Materialized View 节点展示
- 无 Keyspace 级别操作

**修复方案**:
- 创建 `src/lib/tree-adapters/cassandra-adapter.tsx`
- 参考 `sql-adapter.tsx`，添加 Cassandra 特有菜单项
- `driver-registry.tsx` 中将 `treeConfig` 改为 `(callbacks) => createCassandraTreeConfig(callbacks)`

**涉及文件**:
- `src/lib/tree-adapters/cassandra-adapter.tsx`（新建）
- `src/lib/driver-registry.tsx`

---

## 中优先级（功能缺失）

### [ ] 4. `get_table_data` 无 OFFSET 分页

**现象**: 当前只有 `LIMIT`，无 `OFFSET`。数据网格翻页时永远返回相同的前 N 行。CQL 支持 `LIMIT N` 但不原生支持 `OFFSET`（需用 token-based pagination 或 `paging state`）。

**修复方案**:
- 使用 scylla driver 的 `query_single_page` + paging state 实现游标分页
- 或者对于小数据量表使用 `ALLOW FILTERING` + token 范围
- 前端需要传回 paging state 而非 page number

**涉及文件**:
- `src-tauri/src/db/drivers/cassandra.rs` (`get_table_data`)

---

### [ ] 5. `get_table_data` 忽略 filter 参数

**现象**: `_filter` 参数带下划线前缀，完全未使用。数据网格的筛选框输入内容无效。

**修复方案**:
- 将 filter 转为 CQL WHERE 子句
- 注意: 非分区键/聚类列的过滤需要 `ALLOW FILTERING`
- 可选: 仅支持简单条件（`column = value`），复杂条件给出提示

**涉及文件**:
- `src-tauri/src/db/drivers/cassandra.rs` (`get_table_data`)

---

### [ ] 6. 无查询取消支持

**现象**: 未实现 `execute_query_with_id` trait 方法，长时间 CQL 查询无法中断。ClickHouse 和 MySQL 已支持。

**修复方案**:
- 实现 `execute_query_with_id`，使用 tokio `select!` + cancel token
- scylla driver 的 `Session` 支持通过 `query_paged` 的 cancellation token 取消

**涉及文件**:
- `src-tauri/src/db/drivers/cassandra.rs`

---

### [ ] 7. SQL 编辑器无 CQL 关键字补全

**现象**: 编辑器有 ClickHouse 关键字补全，但无 CQL 特有关键字。用户输入 `KEYSPACE`、`CLUSTERING`、`TTL`、`USING` 等无提示。

**修复方案**:
- 在 `src/components/business/Editor/SqlEditor.tsx` 中添加 `CASSANDRA_COMPLETIONS` 数组
- 包含: `KEYSPACE`, `CLUSTERING`, `COMPACT`, `TTL`, `USING`, `CONSISTENCY`, `BATCH`, `COUNTER`, `ALLOW FILTERING`, `TOKEN`, `WRITETIME`, `TTL`

**涉及文件**:
- `src/components/business/Editor/SqlEditor.tsx`

---

### [ ] 8. 连接表单缺少 Cassandra 专属字段

**现象**: 连接对话框无 Cassandra 特有配置项:
- Consistency Level（LOCAL_QUORUM, ONE 等）
- Datacenter 名称（用于 LOCAL_QUORUM 路由）
- SSL 客户端证书支持

**修复方案**:
- 在 `ConnectionDialog.tsx` 中添加 Cassandra 条件字段
- `ConnectionForm` 模型可能需要扩展（或复用 `extra_json`）
- `CassandraDriver::connect` 读取并应用这些配置

**涉及文件**:
- `src/components/business/Sidebar/connection-list/ConnectionDialog.tsx`
- `src-tauri/src/db/drivers/cassandra.rs`

---

### [ ] 9. DDL 生成缺少 WITH 子句

**现象**: `get_table_ddl` 生成的 `CREATE TABLE` 没有 `WITH compaction = ...`、`WITH caching = ...` 等表属性。从其他数据库复制 DDL 时会丢失关键配置。

**修复方案**:
- 在 `get_table_ddl` 中追加从 `system_schema.tables` 查询的 compaction、compression、caching、gc_grace_seconds、default_time_to_live 等属性
- 格式化为 `WITH compaction = {'class': '...'}` 语法

**涉及文件**:
- `src-tauri/src/db/drivers/cassandra.rs` (`get_table_ddl`)

---

## 低优先级（锦上添花）

### [ ] 10. 无独立 Cassandra 命令模块

**说明**: Redis、MongoDB、Elasticsearch 都有独立的 `src-tauri/src/commands/<driver>.rs` 模块。Cassandra 所有操作通过通用 `DatabaseDriver` trait，无法暴露 Cassandra 特有信息。

**可添加的命令**:
- `cassandra_test_connection` — 返回集群名、数据中心、Cassandra 版本、partitioner
- `cassandra_describe_cluster` — 集群拓扑、节点列表、rack 信息
- `cassandra_list_keyspaces` — 包含 replication 详情
- `cassandra_get_table_extra` — 独立命令，无需获取完整 metadata

---

### [ ] 11. DataGrid 无 Cassandra 写入约束检查

**说明**: Cassandra 的 UPDATE/DELETE 必须包含完整分区键，否则报错。DataGrid 的内联编辑功能没有此检查，用户编辑时可能遇到难以理解的错误。

**修复方案**: 在 `TableView.tsx` 中添加 Cassandra 特有的 mutation guard，编辑前检查是否提供了完整主键。

---

### [ ] 12. 树形结构无物化视图展示

**说明**: Cassandra 支持 Materialized View，但当前 `list_tables` 只返回普通表。树形结构中看不到物化视图。

**修复方案**:
- `list_tables` 查询中同时获取 `system_schema.views`
- 树适配器中以不同图标展示视图节点

---

### [ ] 13. `test_connection_ephemeral` 无 Cassandra 专用分支

**说明**: MongoDB、Elasticsearch 有专用的 ephemeral test 命令返回丰富信息（版本、集群名等）。Cassandra 走通用路径，只返回 success/message/latency。

**修复方案**: 创建 `cassandra_test_connection_ephemeral` 命令，返回:
- Cassandra 版本
- 集群名称
- 数据中心名称
- Partitioner
- 节点数量
