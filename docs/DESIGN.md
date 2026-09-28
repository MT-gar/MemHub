# MemHub — 本地 Agent 记忆中枢 · 架构设计与功能规划

> 工作名 **MemHub**（可改）。一句话：把散落在各个 Agent 里的记忆，统一镜像到一个本地 Markdown 目录里，提供 GUI 浏览/搜索/整理，并让你现有的 Agent 通过 MCP 来读取和总结这些记忆——**自身不内置任何 LLM**。

---

## 0. 决策摘要

| 决策点 | 结论 | 理由 |
|---|---|---|
| 接入方式 | 通用优先：**文件夹监听 + MCP Server**，再叠加各 Agent 的专用适配器 | 覆盖面最大；专用适配器只是"预置好的文件夹规则 + 少量路径解码逻辑" |
| 统一记忆的形态 | **纯 Markdown 目录（Vault）+ YAML frontmatter**，SQLite 只做可重建的全文索引 | 人可读、git 可管、任何 Agent 不装插件也能直接 `cat`；索引坏了删掉重建即可 |
| 采集策略 | **单向镜像**（源 → Vault），源文件只读；写回原文件是显式操作 | 不干扰 Claude Auto Dream / Codex consolidation 等 Agent 自己的整理机制 |
| 总结/提炼 | **BYOA（Bring Your Own Agent）**：生成"总结任务包"，由用户已有的 Claude Code / Codex / OpenClaw 等通过 MCP 或 CLI 执行 | 零 API Key、零模型依赖、最轻量；用户用自己已付费/已配置的 Agent |
| 桌面形态 | **Tauri 2**（Rust 后端 + Web 前端），核心逻辑放在独立 Rust crate；另出一个 **`memhub` CLI**（sidecar）承担 MCP/后台同步/浏览器模式 | 安装包 ~10MB；MCP 需要 stdio 子进程，CLI 天然合适；无桌面环境也能用 |
| 发布 | GitHub Actions + `tauri-action` 三平台打包，tag 触发 Release | 现成模板，维护成本低 |

---

## 1. 目标与非目标

**目标**
1. 自动发现并持续镜像本机各 Agent 的记忆文件到统一 Vault（增量、去重、带历史）。
2. 提供桌面 GUI：按 Agent / 项目 / 类型 / 时间 / 标签浏览，全文搜索，Markdown 预览与编辑。
3. 让任意支持 MCP 的 Agent 能搜索、读取、写入统一记忆（跨 Agent 共享记忆）。
4. 一键生成"总结任务"，由用户现有 Agent 执行，产出**知识笔记**（经验、偏好、项目卡片、去重建议），并可回溯到来源记忆。
5. 本地优先、零上传、单机可用；Vault 可用 git 同步到多台机器。

**非目标（至少 v0.x 不做）**
- 不内置 LLM 推理、不要求 API Key。
- 不做向量数据库/RAG 服务（v0.3 作为可选特性评估）。
- 不做多用户权限系统。
- 不代替各 Agent 自己的记忆机制，只做"汇总 + 提炼 + 回流"。

---

## 2. 核心概念

| 概念 | 说明 |
|---|---|
| **Source（记忆源）** | 一个可采集的位置，由 *适配器* 定义：`type`（claude-code / codex / gemini / openclaw / generic / …）、根目录、glob 规则、kind 映射、项目归属规则。 |
| **Memory Entry（记忆条目）** | 归一化后的最小单元 = Vault 中一个 `.md` 文件（正文 + frontmatter）。 |
| **Vault（统一记忆库）** | `~/.memhub/vault/`，纯 Markdown 目录，是唯一真相源。 |
| **Knowledge Note（知识笔记）** | 总结产出的二级产物，存放于 `vault/knowledge/`，frontmatter 记录 `sources` 以回溯。 |
| **Summary Task（总结任务）** | 一次提炼请求 = 选择范围 + 模板 → 任务包（TASK.md + 上下文）→ Agent 执行 → 结果写回。 |
| **kind（条目类型）** | `instruction`（用户给 Agent 的指令，如 CLAUDE.md/AGENTS.md）· `memory`（Agent 自己写的长期记忆）· `daily-log`（按日追加日志）· `session-summary`（会话摘要）· `profile`（用户/人格画像，如 OpenClaw USER.md）· `note`（通过 MCP/API 写入的原始记忆）· `knowledge`（总结产出） |

---

## 3. 数据布局与格式

### 3.1 目录结构

```
~/.memhub/
├── config.toml                 # 全局配置（vault 路径、sources、选项）
├── index.sqlite                # FTS5 全文索引缓存（可随时删除重建）
├── logs/
├── templates/                  # 总结模板（内置模板会在首次启动时复制到这里，可改）
│   ├── lessons.md
│   ├── preferences.md
│   ├── project-brief.md
│   ├── dedupe.md
│   └── digest.md
├── tasks/                      # 总结任务包
│   └── 20260928153000-lessons/
│       ├── TASK.md             # 给 Agent 的完整 prompt：模板 + 范围 + 条目表 + 内联的记忆正文（受预算约束）
│       ├── task.json           # 元数据：范围、模板、状态(pending/done/accepted)、entry_ids
│       └── result.md           # Agent 写回的结果（采纳后复制到 vault/knowledge/）
└── vault/                      # ★ 统一记忆目录（可 git init）
    ├── README.md               # 自动生成：说明结构，方便 Agent 直接读
    ├── agents/                 # 镜像区（源只读）
    │   ├── claude-code/
    │   │   ├── _global/CLAUDE.md
    │   │   └── my-app/                   # 项目 slug
    │   │       ├── MEMORY.md
    │   │       ├── debugging.md
    │   │       └── CLAUDE.md
    │   ├── codex/
    │   │   └── _global/{memory_summary.md, MEMORY.md, rollout_summaries/...}
    │   ├── gemini/_global/GEMINI.md
    │   ├── openclaw/
    │   │   └── main/{MEMORY.md, USER.md, daily/2026-09-27.md, ...}
    │   └── generic/<source-name>/...
    ├── inbox/                  # 通过 MCP / HTTP 写入的记忆（Vault 原生，可编辑）
    │   └── <agent-name>/2026-09-28-<slug>.md
    └── knowledge/              # 总结产出（Vault 原生，可编辑）
        ├── INDEX.md            # 自动维护的目录
        ├── lessons/
        ├── preferences.md
        └── projects/<slug>.md
```

### 3.2 条目 frontmatter

```yaml
---
id: 01JB3Q7Z5K9X8W2N4M6P0R1S2T        # ULID，稳定不变
agent: claude-code                      # 来源 Agent 标识
source: claude-code/auto-memory         # 来源名（适配器内的子类型）
origin: /Users/me/.claude/projects/-Users-me-dev-my-app/memory/MEMORY.md
project: my-app                         # 项目 slug（无则 _global）
project_path: /Users/me/dev/my-app      # 可选，能解析出来时填
kind: memory
title: MEMORY.md                        # 取首个 H1，否则文件名
created: 2026-09-01T10:12:00+08:00
updated: 2026-09-27T22:40:11+08:00
origin_hash: sha256:9f2a…               # 源文件内容哈希，用于增量与去重
tags: []
archived: false                         # 源文件被删时置 true，不物理删除
---
（正文 = 源文件原文，不做改写）
```

- 镜像条目正文**永远等于源文件原文**；GUI 编辑镜像条目时会提示"这是镜像，是否写回源文件"（v0.2 支持写回）。
- `inbox/` 与 `knowledge/` 条目为 Vault 原生，可直接编辑。
- 知识笔记额外字段：`sources: [id, id, …]`、`task: <task-id>`、`template: lessons`。

### 3.3 历史与版本

- 若检测到 `git`，首次运行在 `vault/` 执行 `git init`，每次同步批次后自动 `git commit -m "sync: <n> changed"`（可关闭）。
- 这样"记忆的变迁史"零成本获得，也是多机同步（push 到私有仓库）的基础。

### 3.4 config.toml

```toml
vault = "~/.memhub/vault"
language = "zh-CN"
git_snapshot = true
max_file_size_kb = 2048
redact_secrets = true            # 采集时对疑似密钥打码（正则：sk-…、AKIA…、ghp_… 等）

[[sources]]
type = "claude-code"             # 内置适配器；root 留空 = 自动检测 ~/.claude
enabled = true

[[sources]]
type = "openclaw"
root = "~/.openclaw/workspace"

[[sources]]
type = "generic"
name = "my-langgraph-bot"
root = "~/bots/lg/memory"
include = ["**/*.md", "**/*.txt"]
kind = "memory"

[[projects]]                     # 已登记的项目目录：用于采集项目内 CLAUDE.md / AGENTS.md / .cursor/rules 等
path = "~/dev/my-app"
```

---

## 4. 总体架构

```
┌──────────────────────────────────────────────────────────────┐
│  memhub-desktop (Tauri 2)                                    │
│   ┌──────────────────────┐   invoke    ┌───────────────────┐ │
│   │ Web UI (React + TS)  │◄───────────►│ Tauri commands    │ │
│   └──────────────────────┘             └─────────┬─────────┘ │
└──────────────────────────────────────────────────┼───────────┘
                                                   │ 直接调用
┌──────────────────────────────────────────────────▼───────────┐
│  memhub-core (Rust lib)                                      │
│   adapters ─► normalizer ─► vault(fs) ─► index(SQLite FTS5)  │
│   watcher(notify) · tasks(总结任务包) · git snapshot · config  │
└───────────────┬──────────────────────────────┬───────────────┘
                │                              │
┌───────────────▼───────────────┐  ┌───────────▼───────────────┐
│  memhub CLI (sidecar 二进制)    │  │  HTTP API (axum, 内嵌 UI)  │
│  scan · watch · mcp · serve   │  │  浏览器模式 / 无桌面服务器    │
└───────────────┬───────────────┘  └───────────────────────────┘
                │ MCP over stdio
┌───────────────▼──────────────────────────────────────────────┐
│  Claude Code · Codex · Gemini CLI · OpenClaw · Cursor · 自研   │
└──────────────────────────────────────────────────────────────┘
```

**为什么核心逻辑不放在 Tauri 里**：MCP 需要一个能被 Agent 以 stdio 子进程方式拉起的、启动快、不依赖 GUI 的二进制；服务器/NAS 用户也需要无桌面模式。核心放在 `memhub-core`，Tauri 和 CLI 都只是薄壳。

### 4.1 代码仓库结构（Cargo workspace + pnpm/npm）

```
memhub/
├── Cargo.toml                  # workspace
├── crates/
│   ├── memhub-core/            # 核心库
│   │   └── src/{config,model,frontmatter,vault,index,adapters/,scan,watch,tasks,mcp,gitsnap,redact}.rs
│   └── memhub-cli/             # `memhub` 可执行：scan/watch/mcp/serve
│       └── src/{main,serve}.rs
├── apps/desktop/               # Tauri 2
│   ├── src-tauri/{Cargo.toml, tauri.conf.json, src/main.rs, binaries/}
│   └── (前端在 ui/)
├── ui/                         # React + Vite + TS，Tauri 与浏览器模式共用
│   └── src/{api/, pages/, components/, i18n/}
├── docs/DESIGN.md
├── .github/workflows/{ci.yml, release.yml}
└── README.md
```

---

## 5. 采集层

### 5.1 通用机制（所有适配器共用）

1. **发现**：适配器给出根目录 + `include`/`exclude` glob；启动时全量 `scan`，之后 `watch`（`notify` + 500ms 去抖）增量。
2. **归一化**：读取文件 → 计算 `origin_hash` → 若与 Vault 中同 `origin` 的哈希一致则跳过 → 否则生成/更新条目（保留原 `id`）。
3. **项目归属**：适配器决定 `project`（例如 Claude Code 从 `~/.claude/projects/<编码路径>` 反推；OpenClaw 用 agent 名；generic 用 source 名）。
4. **删除**：源文件消失 → 条目 `archived: true`，正文保留。
5. **安全**：超过 `max_file_size_kb` 跳过；`redact_secrets` 开启时对常见密钥模式打码并在 frontmatter 记 `redacted: true`。
6. **索引**：写入 SQLite（表 `entries` + FTS5 虚表 `entries_fts(title, body, tags)`），支持中文（使用 `trigram` tokenizer，兼顾中英文子串搜索）。

### 5.2 内置适配器（自动检测，来源路径已核实到 2026-09）

| Agent | 检测依据 | 采集内容 → kind | 项目归属 |
|---|---|---|---|
| **Claude Code** | `~/.claude/` 存在 | `~/.claude/CLAUDE.md` → instruction；`~/.claude/projects/*/memory/**/*.md`（Auto Memory：MEMORY.md + 主题文件）→ memory；`~/.claude/agent-memory/*/**/*.md`（子 Agent 持久记忆，user scope）→ memory；已登记项目内 `CLAUDE.md`、`.claude/agent-memory*/` → instruction/memory | `projects/<编码路径>` 目录名 → 反推路径；优先读取该目录下任一 `*.jsonl` 首行的 `cwd` 字段拿到精确路径 |
| **Codex CLI** | `~/.codex/` 存在 | `~/.codex/memories/{memory_summary.md, MEMORY.md}` → memory；`rollout_summaries/*.md` → session-summary；`skills/*/SKILL.md` → memory；`~/.codex/AGENTS.md` 与项目 `AGENTS.md` → instruction（注意 Codex 原生 memories 需 `[features] memories = true` 才会产生） | 全局为 `_global`；项目级来自已登记项目 |
| **Gemini CLI** | `~/.gemini/` 存在 | `~/.gemini/GEMINI.md` → instruction；项目 `GEMINI.md` → instruction | 同上 |
| **OpenClaw** | `~/.openclaw/workspace/`（或 config 指定 agents.defaults.workspace） | `MEMORY.md` → memory；`memory/YYYY-MM-DD.md` → daily-log；`memory/*.md`（非日期）→ memory；`USER.md`/`SOUL.md`/`IDENTITY.md`/`TOOLS.md` → profile（可选） | 多 Agent 工作区时以 agent 名为 project |
| **Cursor** | 项目内 `.cursor/rules/*.mdc`、`.cursorrules` | instruction | 已登记项目 |
| **Windsurf** | `~/.codeium/windsurf/memories/` | memory | `_global` |
| **Cline** | 项目内 `memory-bank/*.md` | memory | 已登记项目 |
| **GitHub Copilot** | 项目 `.github/copilot-instructions.md` | instruction | 已登记项目 |
| **Generic** | 用户手动添加目录 | 按 glob → 用户指定 kind | source 名 |

> 适配器以**声明式**为主（Rust 结构体/内置 TOML 描述 root、globs、kind 映射），仅 Claude Code 的项目路径解码等少量逻辑用代码；新增 Agent 通常只需加一段声明。

**暂不镜像**：完整会话记录（Claude `~/.claude/projects/*/*.jsonl`、Codex `~/.codex/sessions/`）体量大且噪声多；v0.2 计划做"会话摘要抽取"（只取用户消息 + 最终回复片段，作为 `session-summary`），或在总结任务中按需引用。

### 5.3 MCP Server（`memhub mcp`）

- 传输：stdio，JSON-RPC 2.0，实现 `initialize` / `tools/list` / `tools/call` / `ping`。
- 通过 `initialize` 里的 `clientInfo.name` 识别调用方 Agent（也可 `--agent <name>` 覆盖），写入时自动打上 `agent`。

| 工具 | 参数 | 作用 |
|---|---|---|
| `memory_search` | `query, agent?, project?, kind?, limit=10` | FTS 搜索，返回 id/标题/来源/片段 |
| `memory_read` | `id` | 读取完整条目 |
| `memory_list` | `agent?, project?, kind?, since?, limit` | 列表/最近更新 |
| `memory_write` | `title, content, tags?, project?` | 写入 `vault/inbox/<agent>/…`（跨 Agent 共享记忆的入口） |
| `knowledge_save` | `title, content, tags?, sources?` | 直接落一篇知识笔记 |
| `summary_task_get` | `task_id?`（空 = 最早的待处理任务） | 取回任务指令 + 上下文（BYOA 总结的关键） |
| `summary_task_submit` | `task_id, result` | 提交结果，任务置为 `done` 等待 GUI 采纳 |

各 Agent 的接入片段由 GUI「设置」页生成并一键复制，例如：

```jsonc
// Claude Code: claude mcp add memhub -- memhub mcp
// Codex: ~/.codex/config.toml
[mcp_servers.memhub]
command = "memhub"
args = ["mcp"]
// OpenClaw / Cursor / Gemini CLI：同理，command = memhub, args = ["mcp"]
```

### 5.4 HTTP API（`memhub serve`）

采用 **RPC 风格**：`POST /api/<command>`，JSON 进 JSON 出，命令名与 Tauri `invoke` 完全一致（`get_overview`、`search_entries`、`get_entry`、`update_entry`、`create_note`、`create_task`、`submit_task_result`、`accept_task`、`add_source`、`add_project`、`sync_now`、`get_snippets` …，全表见 `memhub_core::api::COMMANDS`）。三种宿主（HTTP / Tauri / MCP）共用 `api::dispatch`，前端只有一个 `call(cmd, params)` 适配层。同时托管编译后的 UI（`rust-embed`）。用途：浏览器模式、无桌面服务器、自研 Agent 直接 HTTP 写入（`POST /api/create_note`）。默认只监听 `127.0.0.1`。

---

## 6. 总结层（BYOA：让你现有的 Agent 来总结）

### 6.1 流程

```
GUI 选范围 + 模板 ──► core 生成任务包 tasks/<id>/{TASK.md, context.md}
        │
        ├─ A. MCP：在任一已接入 Agent 里说「执行 MemHub 总结任务」
        │        → Agent 调 summary_task_get → 产出 → summary_task_submit
        ├─ B. CLI：GUI 给出可复制命令 / 「运行」按钮直接拉起本机 CLI Agent
        │        claude -p "$(cat TASK.md)"  |  codex exec "$(cat TASK.md)"  |  gemini -p …
        └─ C. 手动：复制 TASK.md 到任意聊天窗口，把结果粘回 GUI
        │
        ▼
GUI 预览 result.md → 采纳 → vault/knowledge/<slug>.md（带 sources 回溯）→ 更新 INDEX.md
        │
        └─ 可选：「回流」到 Agent（v0.2）：把提炼的规则追加到 ~/.claude/CLAUDE.md、OpenClaw MEMORY.md 等，带 diff 预览
```

### 6.2 任务包内容

- `TASK.md`：模板渲染结果 = 角色与目标 + 规则（不得编造、引用来源 `[src: id]`、输出语言）+ 输出格式 + 统一页脚（交付方式、范围、条目表、内联正文）。条目按 `updated` 倒序内联，默认预算 200KB（`task_context_budget_kb`），超出的条目只列 id 并提示 Agent 用 `memory_read` 自取。单文件设计是为了 `claude -p "$(cat TASK.md)"` 这类一行命令也能用。
- `task.json`：`{id, template, title, status: pending|done|accepted, created, updated, entry_ids, inlined, task_bytes, scope, knowledge_id}`。
- `result.md`：由 MCP `summary_task_submit`、GUI 粘贴或 CLI 重定向写入；只要文件出现，任务自动进入 `done`。

### 6.3 内置模板

| 模板 | 产出 |
|---|---|
| `lessons` 经验教训 | 从调试记录/踩坑中提炼可复用经验，每条附来源 |
| `preferences` 偏好与规范 | 跨 Agent 一致的用户偏好、代码风格、禁忌 → 可直接粘进任何 AGENTS.md 的通用段落 |
| `project-brief` 项目知识卡 | 每个项目的架构、约定、关键决策、常用命令 |
| `dedupe` 去重与冲突 | 找出各 Agent 间重复/过时/互相矛盾的记忆，给出合并建议 |
| `digest` 周期摘要 | 本周各 Agent 做了什么、学到什么、待办 |

模板是普通 Markdown（`~/.memhub/templates/`），支持 `{{scope}}`、`{{context}}`、`{{date}}` 等占位符，用户可随意改。

---

## 7. GUI 设计

| 页面 | 内容 |
|---|---|
| **概览** | 已检测到的 Agent 与状态、各 Agent 条目数、最近更新流、待处理总结任务、上次同步时间 |
| **记忆库** | 三栏：左 Agent→项目树（含 inbox/knowledge）；中列表（搜索框 + kind/tag/时间筛选，显示标题/项目/更新时间/片段）；右 Markdown 预览 + 编辑（镜像条目提示只读/写回） |
| **知识** | 知识笔记浏览；点击 `sources` 跳回原记忆；按模板/标签分组 |
| **总结任务** | 新建向导（范围 → 模板 → 预览任务包）；任务列表与状态；三种执行方式提示；结果预览与采纳 |
| **来源** | 自动检测结果（含未安装的灰显）、启用/禁用、添加通用目录、添加项目目录、同步日志与错误 |
| **设置** | Vault 路径、git 快照、密钥打码、语言、MCP 接入片段（按 Agent 一键复制）、CLI 路径、导出/备份 |

交互原则：默认全部离线；任何写入源文件的操作都必须显式确认并显示 diff。

---

## 8. 技术选型

**Rust 侧**：Tauri 2（plugins: shell / dialog / opener / updater）、`notify` + `notify-debouncer-mini`、`rusqlite`（bundled + FTS5）、`walkdir` + `globset`、`serde`/`serde_json`、自写极简 frontmatter 解析（避免依赖已归档的 `serde_yaml`）、`ulid`、`chrono`、`sha2`、`clap`、`axum` + `tokio`、`rust-embed`、`tracing`、`anyhow`/`thiserror`。

**前端**：React 18 + TypeScript + Vite、`react-markdown` + `remark-gfm`、CodeMirror 6（编辑）、轻量自写 CSS 变量主题（后续可换 Tailwind + shadcn/ui）、i18n（zh-CN / en）。

**API 适配层**（`ui/src/api/`）：检测 `window.__TAURI_INTERNALS__` → 走 `invoke`；否则走 `/api`（浏览器模式）。两套后端命令签名一致。

---

## 9. 发布与分发

- **CI**：`ci.yml` 跑 `cargo test` + `npm run build`；`release.yml` 在 `v*` tag 上用 `tauri-apps/tauri-action` 构建 macOS（arm64/x64）、Windows x64、Linux x64（.deb/.AppImage），自动创建 GitHub Release 并上传。
- **CLI sidecar**：`memhub` 以 `externalBin` 方式随桌面端打包（`src-tauri/binaries/memhub-<target-triple>`），GUI 首次启动引导把它加到 PATH / 或在 MCP 片段中直接使用绝对路径。
- **单独 CLI**：Release 同时附带 `memhub-<os>-<arch>.tar.gz/zip`，供服务器使用。
- **签名**：macOS 未公证时需右键打开（README 说明）；Windows SmartScreen 提示；后期考虑 Apple Developer / 代码签名。
- **更新**：v0.2 引入 `tauri-plugin-updater`。

---

## 10. 里程碑

| 版本 | 内容 |
|---|---|
| **M0 骨架（本次）** | 设计文档；`memhub-core`（config / model / frontmatter / vault / index / adapters: generic + claude-code + codex + gemini + openclaw / scan / watch / tasks / mcp）；`memhub` CLI（scan · watch · mcp · serve）；Web UI（概览 / 记忆库 / 来源 / 任务 / 设置）；Tauri 壳 + CI 模板 |
| **v0.1.0 能用** | 三平台打包；MCP 7 工具联调（Claude Code / Codex / OpenClaw）；BYOA 总结全流程；git 快照；中英文 |
| **v0.2.0 好用** | 会话摘要抽取；写回源文件与"回流到 Agent"；去重/冲突检测视图；Cursor / Windsurf / Cline / Copilot 适配器；模板编辑器；自动更新 |
| **v0.3.0 生态** | 稳定 HTTP API + Webhook；声明式适配器插件（TOML 即可扩展）；可选本地 embedding 语义搜索（fastembed / Ollama，可选依赖）；多机同步向导（git remote）；Vault 加密备份 |

---

## 11. 风险与对策

| 风险 | 对策 |
|---|---|
| Agent 记忆格式/路径变化 | 声明式适配器，改配置即可；采集失败只记日志不中断 |
| 记忆中含密钥/隐私 | 采集时可选打码；Vault 目录 0700；默认不监听公网、不上传 |
| 文件量大、性能 | 哈希增量、FTS 索引、单文件大小上限、去抖 |
| 与 Agent 自身整理机制冲突（Claude Auto Dream、Codex consolidation） | 单向只读镜像，写回需显式确认 |
| Windows 路径/编码 | 统一用 `dirs::home_dir()`；路径 slug 化；UTF-8 with BOM 处理 |
| Tauri 在 Linux 依赖 webkit2gtk | 提供 AppImage；无桌面环境走 `memhub serve` 浏览器模式 |

---

## 12. 待定项

- 项目名与许可证（建议 MIT 或 Apache-2.0；便于社区贡献适配器）。
- `knowledge/` 是否允许 Agent 通过 MCP 直接改写（默认允许写新文件，不允许覆盖旧文件，改写走 GUI 采纳）。
- 是否把「回流到 Agent」提前到 v0.1（取决于 v0.1 试用反馈）。

---

## 13. M0 交付状态（2026-09-28）

已在沙箱中编译并验证（Rust 1.98，零警告，6 个测试通过，含端到端与 MCP 握手）：

| 模块 | 状态 | 说明 |
|---|---|---|
| `memhub-core` 配置 / 模型 / frontmatter | ✅ | 自写 frontmatter 读写，JSON 子集保证 YAML 兼容 |
| 适配器 | ✅ | claude-code（含 jsonl `cwd` 反推项目路径）、codex、gemini、openclaw（多 workspace）、windsurf、generic、projects（CLAUDE/AGENTS/GEMINI/.cursor/cline/copilot/windsurf/kiro） |
| 同步（scan） | ✅ | 哈希增量、密钥打码、归档、git 快照 |
| 索引 | ✅ | SQLite FTS5 trigram，<3 字回退 LIKE，snippet 高亮 |
| 监听（watch） | ✅ | notify 去抖 + 周期全量 + 根目录热更新 |
| 总结任务 | ✅ | 5 个内置模板、预算内联、pending→done→accepted、采纳生成 knowledge + INDEX.md |
| MCP Server | ✅ | 7 个工具，stdio，按 clientInfo 识别 Agent |
| CLI | ✅ | scan / watch / mcp / serve / detect / search / list / show / paths / reindex |
| HTTP + 内嵌 UI | ✅ | `POST /api/<cmd>`，rust-embed |
| Web UI | ✅ | 概览 / 记忆库 / 知识 / 总结任务 / 来源 / 设置，中英文，暗色自适应 |
| Tauri 壳 | ✅ Windows 通过 | 沙箱无 webkit2gtk 无法编译；2026-09-28 在 Windows 本机（MSVC）`cargo check` 通过并完成 `tauri build`，详见 `docs/mcp-memhub-bootstrap.md`（`rpc` 命令、watcher 事件、`MemHub mcp` 无头模式、sidecar、capabilities、图标全套） |
| CI / Release | ⚠️ 未跑 | `ci.yml`（test + UI build）、`release.yml`（4 平台 tauri-action + CLI 归档） |

Windows 本机验证（cargo build/test、demo 同步、HTTP API、BYOA 闭环、MCP stdio、sidecar、Tauri）与为此做的修正记录在 `docs/mcp-memhub-bootstrap.md`。

下一步建议（进入 v0.1.0）：在本机 `npm run dev --prefix apps/desktop` 打开桌面窗口做一次人工走查，接一次真实的 Claude Code / Codex MCP，推送 GitHub 看 CI，然后打 tag 触发 Release。
