<div align="center">
  <img align="center" src="./public/product-icon.png" width="120" height="120" />
</div>

<h2 align="center">DbPaw</h2>

<div align="center">
<br>
<em>更快的 SQL 编辑与数据探索体验：跨平台、超轻量，AI 助手可选。</em>
<br><br>

[English](README.md) | 简体中文

</div>

<div align="center">

[![GitHub Repo stars](https://img.shields.io/github/stars/codeErrorSleep/dbpaw?style=flat-square)](https://github.com/codeErrorSleep/dbpaw)
[![G-Star](https://atomgit.com/codeErrorSleep/dbpaw/star/badge.svg)](https://atomgit.com/codeErrorSleep/dbpaw)
[![Release](https://img.shields.io/github/v/release/codeErrorSleep/dbpaw?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/releases)
[![Downloads](https://img.shields.io/github/downloads/codeErrorSleep/dbpaw/total?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg?style=flat-square)](https://tauri.app)
<br/>
[![TypeScript](https://img.shields.io/badge/TypeScript-5.8-blue?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-blue?style=flat-square&logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=flat-square)](https://github.com/codeErrorSleep/dbpaw/pulls)

</div>

**DbPaw** 帮助你连接 PostgreSQL、MySQL、MariaDB（MySQL 兼容）、TiDB（MySQL 兼容）、SQLite、SQL Server、ClickHouse（预览版）、DuckDB、StarRocks、Doris、Oracle 与 Redis，高效编写和执行 SQL，并在清爽的桌面 UI 中查看与探索数据。

## ✅ 你可以用它做什么

- 连接 PostgreSQL、MySQL、MariaDB（MySQL 兼容）、TiDB（MySQL 兼容）、SQLite、SQL Server、ClickHouse（预览版，当前只读）、DuckDB、StarRocks、Doris、Oracle 与 Redis（单机 / 集群 / 哨兵）
- 编写与执行 SQL：语法高亮、自动补全、一键格式化
- 在数据网格中浏览结果，支持过滤、排序与分页
- 将表数据和查询结果导出为 **CSV、JSON 或 SQL**（仅 DDL / 仅 DML / DDL+DML），支持当前页、过滤行或全表三种导出范围
- 将整个数据库导出为 SQL 文件（包含结构与数据）
- 将 `.sql` 文件导入 MySQL/MariaDB/TiDB/PostgreSQL/SQLite/DuckDB/SQL Server，失败时全量回滚
- 通过可视化界面**建表与修改表结构**，无需手写 DDL
- 查看表结构（列、类型、主键、索引）及实时 DDL 预览
- 通过 **SQL 执行日志**追踪每条查询，附带耗时与执行状态
- 使用 Saved Queries 保存并复用常用 SQL 脚本
- 使用 AI 侧边栏辅助写 SQL、解释查询（可选）
- 通过 SSH 隧道访问远程数据库
- 浏览与管理 Redis 数据：Key、String、Hash、List、Set、Sorted Set、Stream 及 JSON，支持集群与哨兵模式

## 🖼️ 界面预览

![DbPaw 主工作区](docs/screenshots/01-overview.png.png)

![DbPaw 主工作区（深色模式）](docs/screenshots/01-overview-black.png)

| 连接管理                                     | SQL 编辑器                                    |
| -------------------------------------------- | --------------------------------------------- |
| ![连接管理](docs/screenshots/02-connect.png) | ![SQL 编辑器](docs/screenshots/03-editor.png) |

| 数据网格                                      | AI 助手                                |
| --------------------------------------------- | -------------------------------------- |
| ![数据网格](docs/screenshots/04-ddl-grid.png) | ![AI 助手](docs/screenshots/05-ai.png) |

## ✨ 特性

- **极致轻量**：安装包只有 ≈10MB，安装后占用 ≈80MB，内存常驻极低（甩开 Electron 系工具几条街）。
- **真正现代化**：告别 DBeaver 那种”飞机驾驶舱”式复杂界面，精简掉 99% 开发者一辈子用不上的功能，专注常用场景，操作更直观、更丝滑。
- **跨平台**：支持 macOS、Windows、Linux 多平台（再也不用公司一套、回家一套）。
- **数据库兼容**：当前支持 MySQL、MariaDB（MySQL 兼容）、PostgreSQL、ClickHouse、TiDB、SQL Server、SQLite、DuckDB、StarRocks、Doris、Oracle 与 Redis（单机 / 集群 / 哨兵）——仍在加速适配中。
- **完整数据导入导出**：支持 CSV / JSON / SQL（DDL、DML 或完整）多格式导出，范围可选；支持事务性 SQL 导入与回滚；支持整库 SQL 导出。
- **结构管理**：可视化浏览表结构与 DDL，通过 GUI 建表和改表，无需手写 DDL。
- **颜值在线**：支持超多主题配色（暗黑、浅色、各种高饱和/低饱和风格）。
- **内置 AI 辅助（实验性功能）**：目前支持结合 AI 做 SQL 归纳、表结构解释、慢查询分析等（安全性还在打磨中，后续会加本地/可选云端模式）。
- **完全免费**：不用登录、不用付费、没有会员功能、没有广告。

## 📥 安装

前往 [Releases](https://github.com/codeErrorSleep/dbpaw/releases) 页面下载适合您操作系统的最新版本。

### macOS 用户

1. 从 [Releases](https://github.com/codeErrorSleep/dbpaw/releases) 下载 macOS 版本的 `DbPaw`。
2. 将 `DbPaw.app` 移动到 `/Applications` 文件夹。
3. 打开应用。

如果 macOS 提示“无法识别的开发者”并阻止打开：

1. 打开 **系统设置** → **隐私与安全性**。
2. 滚动到下方 **安全性** 区域，找到关于 `DbPaw` 被阻止的提示。
3. 点击 **仍要打开**，并在弹窗中确认 **打开**。

如果在打开应用时遇到“DbPaw 已损坏”（Gatekeeper 隔离标记）：

1. 将 `DbPaw.app` 移动到 `/Applications` 文件夹。
2. 打开**终端**并运行以下命令：
   ```bash
   sudo xattr -d com.apple.quarantine /Applications/DbPaw.app
   ```
3. 现在可以正常打开应用。

_注意：这是因为应用尚未经过 Apple 公证。_

### Windows 用户

1. 从 [Releases](https://github.com/codeErrorSleep/dbpaw/releases) 下载 Windows 安装包或可执行文件。
2. 运行安装程序/可执行文件。

如果 Windows 弹出“Windows 已保护你的电脑”（SmartScreen）等安全警告：

1. 点击 **更多信息**。
2. 点击 **仍要运行**。

若设备由组织统一管理，可能需要管理员允许该应用运行。

## 🔐 安全与隐私

- DbPaw 是本地桌面应用：数据库连接从你的设备直接访问数据库。
- AI 功能为可选：启用后会向你配置的 AI 服务商发送你的输入、最近对话上下文，以及（可选）schema 概览（表/列/类型等元信息）。
- AI 对话会保存在本地；AI 服务商的 API Key 会在本地加密存储。
- 桌面端未内置遥测/分析 SDK。

## 🛠️ 开发

- 开发指南：[docs/zh/Development/DEVELOPMENT.md](docs/zh/Development/DEVELOPMENT.md)
- 贡献指南：[docs/zh/Community/CONTRIBUTING.md](docs/zh/Community/CONTRIBUTING.md)

## 🏗️ 技术栈

- **核心**: [Tauri v2](https://v2.tauri.app/) (Rust)
- **前端**: [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/)
- **样式**: [TailwindCSS v4](https://tailwindcss.com/), [Shadcn/UI](https://ui.shadcn.com/)
- **状态管理**: React Hooks & Context
- **编辑器**: [Monaco Editor](https://microsoft.github.io/monaco-editor/) / CodeMirror

## 📄 许可证

本项目采用 Apache License 2.0 许可证 - 详情请参阅 [LICENSE](LICENSE) 文件。

## ❤️ 致谢

感谢你试用 DbPaw。若它对你有帮助，欢迎给本仓库点个 Star 支持一下，这对项目发展很重要！
