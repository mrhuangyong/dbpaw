# DbPaw MCP 快速接入指南

让 Claude、Cursor 等 AI 助手直接查询你的数据库。

## 快速开始

### 1. 编译

```bash
cd src-tauri && cargo build --bin dbpaw-mcp
```

### 2. 配置 AI 助手

**Claude Desktop** — 编辑 `~/Library/Application Support/Claude/claude_desktop_config.json`：

```json
{
  "mcpServers": {
    "dbpaw": {
      "command": "/你的路径/src-tauri/target/debug/dbpaw-mcp"
    }
  }
}
```

**Cursor** — 编辑 `~/.cursor/mcp.json`，格式相同。

### 3. 重启 AI 助手

完成。现在可以这样对话：

- "列出我的数据库连接"
- "查询 users 表前 10 条"
- "描述 orders 表结构"
- "获取 mydb 的 schema 上下文，帮我写个统计 SQL"

## 安全设置

默认只读，禁止写操作和危险操作。如需开启：

```json
{
  "mcpServers": {
    "dbpaw": {
      "command": "...",
      "env": {
        "DBPAW_MCP_ALLOW_WRITES": "1",
        "DBPAW_MCP_ALLOW_DANGEROUS": "0",
        "DBPAW_MCP_MAX_ROWS": "100"
      }
    }
  }
}
```

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `DBPAW_MCP_ALLOW_WRITES` | `0` | 允许 INSERT/UPDATE/DELETE |
| `DBPAW_MCP_ALLOW_DANGEROUS` | `0` | 允许 DROP/TRUNCATE/ALTER |
| `DBPAW_MCP_MAX_ROWS` | `100` | 查询最大返回行数 |

## 支持的数据库

PostgreSQL、MySQL、MariaDB、TiDB、SQLite、SQL Server、ClickHouse、DuckDB、Oracle

## 常见问题

**Q: AI 提示找不到工具？**
确认配置文件路径正确，重启 AI 助手。

**Q: 查询报错 "Write operation not allowed"？**
默认只读，需设置 `DBPAW_MCP_ALLOW_WRITES=1`。

**Q: UPDATE/DELETE 被拒绝？**
必须带 WHERE 子句，防止误操作全表更新。

---

详细文档：[docs/mcp.md](./mcp.md)
