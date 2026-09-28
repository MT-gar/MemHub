<p align="center">
  <img src="apps/desktop/src-tauri/icons/128x128@2x.png" width="96" alt="MemHub" />
</p>
<h1 align="center">MemHub</h1>
<p align="center"><b>把所有 AI Agent 的记忆，统一保管在本地的一个目录里。</b><br/>
Claude Code · Codex CLI · Gemini CLI · OpenClaw · Cursor · Windsurf · 自研 Agent</p>
<p align="center"><a href="README.md">English</a> · <a href="docs/DESIGN.md">设计文档</a></p>

---

每个 Agent 都有自己的小本本：`CLAUDE.md`、`~/.claude/projects/*/memory/`、`~/.codex/memories/`、OpenClaw 的 `MEMORY.md` 和每日日志、`.cursor/rules`…… MemHub 会自动找到它们，**单向镜像到本地一个纯 Markdown 的 Vault**，提供桌面 GUI 浏览 / 搜索 / 编辑，通过 **MCP** 让每个 Agent 都能读写这份共享记忆，并把"总结我的 Agent 都学到了什么"变成一个由**你现有的 Agent** 来执行的任务——不需要 API Key、不内置 LLM、任何数据不出本机。

## 功能

- **自动发现 + 单向镜像**：增量哈希、源文件删除后归档、可选 git 快照保留历史。
- **内置适配器**：Claude Code（全局/项目 `CLAUDE.md`、Auto Memory、子 Agent 记忆）、Codex CLI（`memories/`、`AGENTS.md`）、Gemini CLI、OpenClaw（长期记忆、每日日志、画像文件）、Windsurf；项目级文件（`AGENTS.md`、`.cursor/rules`、Cline `memory-bank/`、Copilot instructions、Kiro steering）；以及**任意目录**（自研 Agent）。
- **GUI**（Tauri 2）：概览、三栏记忆浏览器 + 全文搜索（SQLite FTS5，trigram 分词对中文友好）、知识笔记、总结任务、来源、设置；中英双语。
- **MCP Server**（`memhub mcp`）：`memory_search` / `memory_read` / `memory_list` / `memory_write` / `knowledge_save` / `summary_task_get` / `summary_task_submit`，Agent 之间通过 Vault 共享记忆。
- **BYOA 总结**：选范围 + 模板（经验教训 / 偏好规范 / 项目卡片 / 去重冲突 / 周期摘要）→ 生成任务包 → 用 MCP、命令行一行（`claude -p …` / `codex exec …` / `gemini -p …`）或手动复制粘贴执行 → 预览 → 采纳为带来源回溯的知识笔记。
- **默认隐私**：纯本地；镜像时对疑似密钥打码；Vault 目录 `0700`。
- **浏览器模式**：`memhub serve`，服务器 / NAS 也能用同一套 UI。

## 安装

到 [Releases](../../releases) 下载桌面安装包（`.dmg` / `.msi` / `.AppImage` / `.deb`）或独立 CLI。
macOS 版本暂未签名：首次打开请右键 → 打开。

从源码构建：

```bash
# CLI + 浏览器模式
npm ci --prefix ui && npm run build --prefix ui
cargo build --release -p memhub-cli
./target/release/memhub serve            # → http://127.0.0.1:7337

# 桌面版（需要 Tauri 环境：https://tauri.app/start/prerequisites/）
./scripts/build-sidecar.sh               # 把 CLI 打进 sidecar
npm ci --prefix apps/desktop && npm run build --prefix apps/desktop
```

> Windows：请使用 MSVC 工具链（`rustup default stable-msvc`，或在仓库目录执行
> `rustup override set stable-x86_64-pc-windows-msvc`）。GNU 工具链缺少 `dlltool` 时会在编译
> `windows-sys` 时失败。脚本（`scripts/*.sh`）在 Git Bash 中可直接运行。

## 接入 Agent（MCP）

「设置 → MCP 接入」页会为每个 Agent 生成可复制的片段。要点：

```bash
claude mcp add --scope user memhub -- memhub mcp          # Claude Code
codex mcp add memhub -- memhub mcp                        # Codex CLI
gemini mcp add memhub memhub mcp                          # Gemini CLI
# Cursor / Windsurf / Claude Desktop / OpenClaw: {"mcpServers":{"memhub":{"command":"memhub","args":["mcp"]}}}
```

不支持 MCP 的 Agent 直接读目录即可：把「直接访问目录」片段贴进任意 `AGENTS.md` / `CLAUDE.md`。

## 总结是怎么跑的

1. **总结任务 → 新建**：选模板和范围（Agent、项目、类型、关键词、时间）。MemHub 生成 `~/.memhub/tasks/<id>/TASK.md`，把指令和选中的记忆内联进去（超出预算的只列 id，Agent 可用 `memory_read` 自取）。
2. 用你已经在用的 Agent 执行：
   - 在任一接入了 MCP 的 Agent 里说「执行 MemHub 总结任务 `<id>`」→ Agent 调 `summary_task_get` … `summary_task_submit`；
   - 或 `claude -p "$(cat TASK.md)" > result.md`（`codex exec` / `gemini -p` 同理）；
   - 或把 `TASK.md` 贴进任意聊天窗口，再把回答贴回来。
3. 在 App 里预览 → **采纳** → 生成 `vault/knowledge/<模板>/<日期>-<标题>.md`（frontmatter 里 `sources: [ids]` 可回溯），并重建 `knowledge/INDEX.md`。Agent 可直接读文件或走 MCP。

## Vault 结构

```
~/.memhub/
├── config.toml           来源、项目、选项
├── index.sqlite          全文索引缓存（可随时删除重建）
├── templates/            总结模板（可编辑的 Markdown）
├── tasks/<id>/           TASK.md · task.json · result.md
└── vault/                ★ 统一记忆目录（有 git 时自动 git init）
    ├── agents/<agent>/<project>/…   只读镜像
    ├── inbox/<agent>/…              通过 MCP / HTTP 写入的笔记
    └── knowledge/…                  采纳的总结 + INDEX.md
```

每个条目都是 Markdown + 一小段 YAML frontmatter（`id`、`agent`、`source`、`origin`、`project`、`kind`、`title`、`created`、`updated`、`origin_hash`、`tags`、`archived`、`sources`…）。MemHub 从不改写源文件；改原文件，MemHub 会重新同步。

## CLI

```
memhub scan                 镜像一次
memhub watch                镜像后持续监听
memhub mcp [--agent NAME]   stdio MCP Server
memhub serve [--port 7337]  Web UI + HTTP API（POST /api/<command>）
memhub detect | list | search <q> | show <id> | paths | reindex
```

`MEMHUB_HOME`（默认 `~/.memhub`）可整体搬家；`MEMHUB_USER_HOME` 可改变适配器扫描的用户目录
（默认为系统主目录，Windows 下忽略 `HOME`），两者配合即可做测试与演示（`scripts/demo.sh`）。

## 仓库结构

```
crates/memhub-core   Rust 核心库：适配器、Vault、索引、任务、MCP、监听、API 分发
crates/memhub-cli    `memhub` 可执行文件（同时是桌面版 sidecar）
ui/                  React + Vite 前端，桌面版与浏览器模式共用
apps/desktop         Tauri 2 壳（很薄：一个 `rpc` 命令 → core）
docs/DESIGN.md       架构与路线图
scripts/             演示数据、开发循环、sidecar 构建
```

## 路线图

见 [docs/DESIGN.md](docs/DESIGN.md#10-里程碑)：会话记录摘要、带 diff 预览的写回、去重/冲突视图、更多适配器、自动更新。

## 许可证

MIT
