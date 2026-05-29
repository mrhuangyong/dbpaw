# 数据库支持分析报告

## 当前已支持的14种数据库

### SQL数据库 (11种)

| 数据库 | 类型 | 流行度排名 |
|--------|------|-----------|
| PostgreSQL | 关系型 | #4 |
| MySQL | 关系型 | #2 |
| MariaDB | 关系型 | #12 |
| TiDB | 分布式SQL | #20+ |
| StarRocks | 分析型 | 新兴 |
| Apache Doris | 分析型 | 新兴 |
| SQLite | 嵌入式 | #9 |
| DuckDB | 分析型 | 新兴 |
| ClickHouse | 列式 | #17 |
| SQL Server | 关系型 | #3 |
| Oracle | 关系型 | #1 |

### 非SQL数据源 (3种)

| 数据库 | 类型 | 流行度排名 |
|--------|------|-----------|
| MongoDB | 文档型 | #5 |
| Redis | 键值型 | #6 |
| Elasticsearch | 搜索引擎 | #7 |

---

## 按流行度推荐添加的数据库

### 第一梯队 (高流行度，强烈推荐)

1. **IBM Db2** - 企业级关系型数据库
   - 流行度排名: #8
   - 应用场景: 大型企业、金融、银行
   - 实现难度: 中等 (有Rust驱动 `ibm_db2`)

2. **Cassandra** - 分布式列式数据库
   - 流行度排名: #11
   - 应用场景: 大数据、物联网、时序数据
   - 实现难度: 中等 (有Rust驱动 `cdrs`)

3. **DynamoDB** - AWS NoSQL数据库
   - 流行度排名: #13
   - 应用场景: 云原生、AWS生态
   - 实现难度: 高 (需要AWS SDK)

### 第二梯队 (中等流行度，建议支持)

4. **Couchbase** - 文档数据库
   - 流行度排名: #14
   - 应用场景: 缓存、文档存储
   - 实现难度: 中等

5. **Neo4j** - 图数据库
   - 流行度排名: #15
   - 应用场景: 社交网络、推荐系统
   - 实现难度: 中等 (有Rust驱动 `neo4j`)

6. **InfluxDB** - 时序数据库
   - 流行度排名: #16
   - 应用场景: 监控、物联网、时序数据
   - 实现难度: 低 (有Rust驱动 `influxdb`)

### 第三梯队 (新兴/云数据库，可选支持)

7. **CockroachDB** - 分布式SQL数据库
   - 流行度排名: #19
   - 应用场景: 分布式系统、云原生
   - 实现难度: 低 (PostgreSQL兼容)

8. **Snowflake** - 云数据仓库
   - 流行度排名: #18
   - 应用场景: 数据分析、BI
   - 实现难度: 高 (需要专用协议)

9. **PlanetScale** - MySQL兼容的无服务器数据库
   - 流行度排名: 新兴
   - 应用场景: 现代Web应用
   - 实现难度: 低 (MySQL兼容)

10. **Neon** - 无服务器PostgreSQL
    - 流行度排名: 新兴
    - 应用场景: 现代Web应用
    - 实现难度: 低 (PostgreSQL兼容)

---

## 实现建议

### 优先级排序

1. **InfluxDB** - 时序数据库需求大，实现简单
2. **Cassandra** - 大数据场景常用
3. **Neo4j** - 图数据库独特场景
4. **CockroachDB** - 分布式SQL，PostgreSQL兼容
5. **IBM Db2** - 企业级市场

### 技术考虑

- PostgreSQL兼容的数据库 (CockroachDB, Neon) 可以复用现有PostgreSQL驱动
- MySQL兼容的数据库 (PlanetScale) 可以复用现有MySQL驱动
- 新协议数据库需要开发新驱动

---

## 参考资源

- [DB-Engines 数据库排名](https://db-engines.com/en/ranking)
- [项目数据库驱动目录](../src-tauri/src/db/drivers/)
- [添加新数据库文档](../ADD_NEW_DB.md)
