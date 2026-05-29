# Redis 实现评估与后续行动追踪

> 评估日期：2026-04-25
> 更新日期：2026-05-18
> 评估范围：后端 datasource/command、前端视图、集成测试、架构可扩展性

---

## 一、已实现功能清单（✅ 已完成）

| # | 功能模块 | 说明 | 验证方式 |
|---|---------|------|---------|
| 1 | 单机连接 | Standalone 模式，host:port 连接 | 集成测试 + 手动测试 |
| 2 | Cluster 连接 | 逗号分隔 seed nodes 隐式识别 | 集成测试 `cluster_scan_requires_narrow_pattern` |
| 3 | 密码/ACL 认证 | 无密码、单密码、username+password | 手动测试 |
| 4 | SSH 隧道 | 默认目标端口 6379 | 配置层面支持 |
| 5 | Key 浏览（SCAN） | 单机 `SCAN` cursor 分页 | 集成测试 |
| 6 | Cluster-aware SCAN | 各 master 独立 cursor，base64 状态聚合 | 集成测试 |
| 7 | Pattern 搜索 | 支持 `*` `?` 等通配符 | 手动测试 |
| 8 | Cluster wildcard 安全限制 | 空 pattern 或纯 `*` 返回 `[VALIDATION_ERROR]` | 集成测试 |
| 9 | String CRUD | GET / SET / DEL / RENAME / TTL | 集成测试 |
| 10 | Hash CRUD + 分页 | HSCAN 分页，字段级增删改（patch） | 集成测试 |
| 11 | List CRUD + 分页 | LRANGE 分页，项级 LSET/LREM/LPUSH/RPUSH/LPOP/RPOP | 集成测试 |
| 12 | Set CRUD + 分页 | SSCAN 分页，成员级 SADD/SREM（patch） | 集成测试 |
| 13 | ZSet CRUD + 分页 | ZRANGE 分页，成员+score 级 ZADD/ZREM（patch） | 集成测试 |
| 14 | 大 Value 分页加载 | 前端 "Load more" + 后端 `get_key_page` | 手动测试 |
| 15 | TTL 管理 | 设置 TTL / PERSIST，前端校验正整数范围 | 手动测试 |
| 16 | Rename 覆盖确认 | `RENAME`/`RENAMENX` + `force` 参数 + 弹窗确认 | 集成测试 |
| 17 | Binary 安全 | 非法 UTF-8 检测，Base64/文本切换，保存二次确认 | 集成测试 |
| 18 | Redis Console | 命令历史、上下键导航、引号/转义解析、格式化输出 | 手动测试 |
| 19 | 危险操作确认 | Delete / Overwrite / Binary overwrite / Force rename | 手动测试 |
| 20 | 连接缓存与自动重连 | `RedisConnectionCache` 按 `connection_id:db` 缓存，IO 错误驱逐 | 代码审查 |
| 21 | 统一错误前缀 | `[REDIS_ERROR]` / `[VALIDATION_ERROR]` / `[UNSUPPORTED]` | 代码审查 |
| 22 | 单元测试 | 连接解析、命令 tokenize、value format | `cargo test` |
| 23 | 集成测试 | 单机 CRUD、分页、Cluster scan、TTL、rename | `redis_integration.rs` |
| 24 | Redis 操作日志 | 记录执行的 Redis 命令，自动保留最新 100 条 | 手动测试 |

---

## 二、缺失功能追踪清单（按优先级）

### 🔴 P0 — 高级 Redis 类型支持

| # | 功能 | 状态 | 优先级 | 备注 |
|---|------|------|--------|------|
| 2.1 | **Stream 类型查看** | ✅ 已完成 | 🔴 P0 | 条目列表、ID 范围、消费者组 |
| 2.2 | **RedisJSON 类型查看** | ✅ 已完成 | 🔴 P0 | JSON 格式化展示与编辑 |
| 2.3 | **Bitmap 专用视图** | ✅ 已完成 | 🟠 P1 | 网格可视化、SETBIT/GETBIT/BITCOUNT/BITPOS |
| 2.4 | **Geo 专用视图** | ✅ 已完成 | 🟠 P1 | GEOPOS/GEODIST/GEOADD/GEOSEARCH |
| 2.5 | **HyperLogLog 专用视图** | ✅ 已完成 | 🟠 P1 | PFCOUNT/PFADD、估算基数展示 |

### 🟠 P1 — 部署架构与连接模型

| # | 功能 | 状态 | 优先级 | 备注 |
|---|------|------|--------|------|
| 2.6 | **Sentinel 模式支持** | ✅ 已完成 | 🟠 P1 | 完整实现，sentinel_password 已修复 |
| 2.7 | **显式连接模式选择 UI** | ✅ 已完成 | 🟠 P1 | standalone/cluster/sentinel 下拉选择 + 动态表单 |
| 2.8 | **连接选项结构化** | ✅ 已完成 | 🟡 P2 | `mode`/`seedNodes`/`sentinels`/`connectTimeoutMs`/`serviceName`/`sentinelPassword` |
| 2.9 | **SSL CA 证书支持** | ❌ 未开始 | 🟡 P2 | 当前 `supportsSSLCA: false` |

### 🟡 P2 — 数据管理与操作能力

| # | 功能 | 状态 | 优先级 | 备注 |
|---|------|------|--------|------|
| 2.10 | **批量删除** | ❌ 未开始 | 🟡 P2 | 按 pattern 或多选 key 删除 |
| 2.11 | **批量 TTL 修改** | ❌ 未开始 | 🟡 P2 | 同上 |
| 2.12 | **数据导入** | ❌ 未开始 | 🟡 P2 | Redis 数据恢复能力 |
| 2.13 | **数据导出** | ❌ 未开始 | 🟡 P2 | Redis 数据备份能力 |
| 2.14 | **只读模式** | ❌ 未开始 | 🟡 P2 | 连接级只读开关 |
| 2.15 | **操作日志/审计** | ✅ 已完成 | 🟡 P2 | 记录执行的 Redis 命令，自动保留最新 100 条 |
| 2.16 | **保存 Console 命令** | ❌ 未开始 | 🟡 P2 | 类似 SQL 的 SavedQuery |

### 🟢 P3 — AI 与其他增强

| # | 功能 | 状态 | 优先级 | 备注 |
|---|------|------|--------|------|
| 2.17 | **AI 辅助（Redis 场景）** | ❌ 未开始 | 🟢 P3 | 命令生成、数据解释等 |
| 2.18 | **Key 内存分析** | ❌ 未开始 | 🟢 P3 | `MEMORY USAGE` 展示 |
| 2.19 | **慢查询查看** | ❌ 未开始 | 🟢 P3 | `SLOWLOG GET` 集成 |

---

## 三、架构债务追踪

| # | 问题 | 影响范围 | 建议方案 | 状态 |
|---|------|---------|---------|------|
| 3.1 | Sidebar 树硬编码适配 | `ConnectionList.tsx` 大量 `type === "redis"` 分支 | 抽象通用 datasource tree node，剥离到 `RedisTreeNode` | ❌ 未开始 |
| 3.2 | 集成测试依赖本地实例 | 需手动启动 Redis 单机+Cluster | 提供 Docker Compose 一键测试环境 | ❌ 未开始 |
| 3.3 | CI 未覆盖 Redis 测试 | `.github/workflows/ci.yml` 无 Redis job | 加入 Redis 测试 job（可用 Docker 服务） | ❌ 未开始 |

---

## 四、与 SQL 数据库的功能对照表

| 功能维度 | MySQL/PostgreSQL/等 | Redis | Redis 差距 |
|---------|---------------------|-------|-----------|
| 查询编辑器 | ✅ SQL 编辑器（高亮、补全、AI） | 🟡 Console（命令行） | 无 AI 辅助 |
| 数据网格 | ✅ 完整 TableView | ❌ 不适用（KV 模型） | — |
| Schema 浏览 | ✅ DB → Schema → Table → Column/Index | 🟡 DB → Key | 无 schema/表/列概念 |
| 表结构查看 | ✅ 列信息、索引、外键、DDL | ❌ 无 | Redis 无表元数据 |
| AI 功能 | ✅ SQL 生成/优化/解释 | ❌ 无 | 完全缺失 |
| 导入/导出 | ✅ CSV/JSON/SQL/全量导出 | ❌ 无 | 完全缺失 |
| 保存查询 | ✅ SavedQuery | ❌ 无 | Console 命令不支持保存 |
| 创建数据库 | ✅ 部分驱动支持 | ❌ 无 | Redis DB 是数字索引 |
| 连接模式选择 | ✅ 标准主机+端口+数据库 | ✅ 显式选择 | standalone/cluster/sentinel 下拉 |
| SSL CA 证书 | ✅ 多数驱动支持 | ❌ 不支持 | `supportsSSLCA: false` |
| 只读模式 | ✅ 部分支持 | ❌ 无 | 缺失 |
| 操作日志 | ✅ SQL 执行日志 | ❌ 无 | 缺失 |

---

## 五、推荐推进路线图

```
第一阶段（安全与可用性）— ✅ 全部完成
  ├─ Cluster wildcard 后端限制 ✅
  ├─ 二进制数据安全策略 ✅
  └─ List 项级编辑 ✅

第二阶段（质量保障）— 🟡 进行中
  ├─ 容器化测试环境 + CI 覆盖 ← 建议优先推进
  └─ Rename 强制覆盖选项 ✅

第三阶段（功能扩展）— ✅ 全部完成
  ├─ Stream 基础查看 ✅
  ├─ RedisJSON 格式化展示与编辑 ✅
  ├─ Bitmap 专用视图（网格可视化、SETBIT/GETBIT/BITCOUNT/BITPOS）✅
  ├─ HyperLogLog 专用视图（PFCOUNT/PFADD、估算基数展示）✅
  └─ Geo 专用视图（GEOPOS/GEODIST/GEOADD/GEOSEARCH）✅

第四阶段（架构升级）— 🟡 进行中
  ├─ 显式连接模式（Sentinel 支持）✅
  ├─ 连接选项结构化（mode/seedNodes/sentinels）✅
  ├─ 修复 sentinel_password 未传递问题 ✅
  └─ Sidebar 树结构抽象（为 MongoDB/ES 预留）

第五阶段（数据管理）— ❌ 未开始
  ├─ 批量操作（删除/TTL）
  ├─ 导入/导出
  └─ 只读模式 + 操作日志
```

---

## 六、关键文件索引

| 文件 | 作用 |
|------|------|
| `src-tauri/src/datasources/redis.rs` | Redis 原生能力层：连接、扫描、CRUD、分页、原始命令执行 |
| `src-tauri/src/commands/redis.rs` | Tauri command 层：连接缓存、IO 错误重连、校验 |
| `src/components/business/Redis/RedisBrowserView.tsx` | 左侧 Key 浏览器：搜索、SCAN 分页、Load more |
| `src/components/business/Redis/RedisKeyView.tsx` | 右侧 Key 编辑器：创建、编辑、TTL、危险确认、patch 构建 |
| `src/components/business/Redis/RedisConsole.tsx` | Redis 命令行控制台 |
| `src/components/business/Redis/value-viewer/*.tsx` | 各类型专用编辑器（string/hash/list/set/zset） |
| `src/components/business/Redis/redis-utils.ts` | TTL 解析、score 校验、分页状态判断 |
| `src/components/business/Sidebar/ConnectionList.tsx` | Sidebar 树：Redis key 加载、路由、Context menu |
| `src/services/api.ts` | 前后端 API 契约 |
| `src-tauri/tests/redis_integration.rs` | 集成测试（单机 + Cluster） |
| `docs/zh/Development/REDIS_IMPLEMENTATION_STATUS.md` | 原始实现状态文档 |

---

## 七、待完成工作总结（2026-05-18 更新）

### 🔧 Bug 修复（优先级高）

| # | 问题 | 文件位置 | 状态 |
|---|------|---------|------|
| 1 | `sentinel_password` 未传递 | `src-tauri/src/datasources/redis.rs:973` | ✅ 已修复 |

### 🟡 P2 — 数据管理与操作能力（6 项）

| # | 功能 | 状态 | 说明 |
|---|------|------|------|
| 2.10 | 批量删除 | ❌ 未开始 | 按 pattern 或多选 key 删除 |
| 2.11 | 批量 TTL 修改 | ❌ 未开始 | 批量设置过期时间 |
| 2.12 | 数据导入 | ❌ 未开始 | Redis 数据恢复能力 |
| 2.13 | 数据导出 | ❌ 未开始 | Redis 数据备份能力 |
| 2.14 | 只读模式 | ❌ 未开始 | 连接级只读开关 |
| 2.15 | 操作日志/审计 | ✅ 已完成 | 记录执行的 Redis 命令，自动保留最新 100 条 |
| 2.16 | 保存 Console 命令 | ❌ 未开始 | 类似 SQL 的 SavedQuery |

### 🟢 P3 — AI 与其他增强（3 项）

| # | 功能 | 说明 |
|---|------|------|
| 2.17 | AI 辅助（Redis 场景） | 命令生成、数据解释等 |
| 2.18 | Key 内存分析 | `MEMORY USAGE` 展示 |
| 2.19 | 慢查询查看 | `SLOWLOG GET` 集成 |

### 🏗️ 架构债务（3 项）

| # | 问题 | 建议方案 |
|---|------|---------|
| 3.1 | Sidebar 树硬编码适配 | 抽象通用 datasource tree node，剥离到 `RedisTreeNode` |
| 3.2 | 集成测试依赖本地实例 | 提供 Docker Compose 一键测试环境 |
| 3.3 | CI 未覆盖 Redis 测试 | 加入 Redis 测试 job（可用 Docker 服务） |

### 🔒 安全与连接（1 项）

| # | 功能 | 说明 |
|---|------|------|
| 2.9 | SSL CA 证书支持 | 当前 `supportsSSLCA: false` |
