# DbPaw MCP Server

DbPaw 现在支持 MCP (Model Context Protocol) Server，可以让 AI 助手（如 Claude、Cursor）直接访问和查询您的数据库。

## 功能特性

### 支持的工具

| 工具 | 描述 |
|------|------|
| `dbpaw_list_connections` | 列出所有保存的数据库连接 |
| `dbpaw_list_databases` | 列出指定连接的所有数据库 |
| `dbpaw_list_tables` | 列出数据库中的所有表 |
| `dbpaw_describe_table` | 获取表结构（列、索引、外键） |
| `dbpaw_get_ddl` | 获取表的 CREATE TABLE DDL |
| `dbpaw_get_schema_context` | 获取 Schema 上下文（给 AI 写 SQL 用） |
| `dbpaw_execute_query` | 执行 SQL 查询 |

### 支持的数据库

- PostgreSQL
- MySQL / MariaDB / TiDB
- SQLite
- SQL Server
- ClickHouse
- DuckDB
- Oracle

## 快速开始

### 1. 编译 MCP Server

```bash
cd src-tauri
cargo build --bin dbpaw-mcp
```

### 2. 配置 AI 助手

#### Claude Desktop

运行配置脚本：

```bash
./scripts/setup-mcp.sh
```

或手动编辑配置文件：

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "dbpaw": {
      "command": "/path/to/dbpaw/src-tauri/target/debug/dbpaw-mcp",
      "args": []
    }
  }
}
```

#### Cursor

运行配置脚本：

```bash
./scripts/setup-mcp.sh
```

或手动编辑配置文件：

**macOS/Linux**: `~/.cursor/mcp.json`

```json
{
  "mcpServers": {
    "dbpaw": {
      "command": "/path/to/dbpaw/src-tauri/target/debug/dbpaw-mcp",
      "args": []
    }
  }
}
```

### 3. 重启 AI 助手

重启 Claude Desktop 或 Cursor 以加载新的 MCP Server 配置。

## 使用示例

### 列出所有连接

```
请帮我列出 DbPaw 中所有的数据库连接
```

### 查询数据库

```
请查询 production 数据库中 users 表的前 10 条记录
```

### 获取表结构

```
请描述 orders 表的结构
```

### 获取 Schema 上下文

```
请获取 mydb 数据库的 schema 上下文，我需要写 SQL 查询
```

## 安全设置

### 默认行为

默认情况下，MCP Server 处于**只读模式**，禁止写操作和危险操作。

### 环境变量

| 变量 | 默认值 | 描述 |
|------|--------|------|
| `DBPAW_MCP_ALLOW_WRITES` | `0` | 设为 `1` 允许 INSERT/UPDATE/DELETE |
| `DBPAW_MCP_ALLOW_DANGEROUS` | `0` | 设为 `1` 允许 DROP/TRUNCATE/ALTER |
| `DBPAW_MCP_MAX_ROWS` | `100` | 查询结果最大返回行数 |

### 配置示例

在 Claude Desktop 配置中添加环境变量：

```json
{
  "mcpServers": {
    "dbpaw": {
      "command": "/path/to/dbpaw-mcp",
      "args": [],
      "env": {
        "DBPAW_MCP_ALLOW_WRITES": "1",
        "DBPAW_MCP_MAX_ROWS": "50"
      }
    }
  }
}
```

## SQL 安全检查

MCP Server 包含多层 SQL 安全检查：

1. **空检查**：拒绝空 SQL 语句
2. **单语句检查**：拒绝多语句（防止 `;` 注入）
3. **危险关键字检查**：拒绝 DROP/TRUNCATE/ALTER（除非启用）
4. **只读检查**：拒绝写操作（除非启用）
5. **WHERE 强制**：UPDATE/DELETE 必须有 WHERE 子句

## 故障排除

### MCP Server 无法启动

1. 检查二进制文件是否存在
2. 确认有执行权限：`chmod +x dbpaw-mcp`
3. 检查是否有依赖问题

### AI 助手无法连接

1. 确认配置文件路径正确
2. 重启 AI 助手
3. 检查 MCP Server 进程是否在运行

### 查询失败

1. 检查连接配置是否正确
2. 确认数据库连接可用
3. 检查 SQL 是否符合安全策略

## 开发说明

### 目录结构

```
src-tauri/src/mcp/
├── main.rs          # 独立二进制入口
├── mod.rs           # 模块入口
├── server.rs        # MCP Server 核心
├── handler.rs       # 请求处理
├── transport.rs     # stdio 传输层
├── types.rs         # 协议类型
├── sql_safety.rs    # SQL 安全检查
└── tools/
    ├── mod.rs       # 工具注册
    ├── connection.rs # 连接管理
    ├── schema.rs    # Schema 上下文
    └── sql.rs       # SQL 查询
```

### 扩展工具

要添加新的 MCP 工具：

1. 在 `tools/mod.rs` 中注册工具定义
2. 实现工具函数
3. 在 `tools/mod.rs` 的 `execute_tool` 函数中添加路由

### 测试

```bash
# 测试 initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/debug/dbpaw-mcp

# 测试 tools/list
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/dbpaw-mcp
```
