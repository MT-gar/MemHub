# MemHub 下一步开发方向分析（v0.1.0 之后）

> 日期：2026-10-01 · 基线：`v0.1.0`（`ba4eda3`）· 本文只做分析与规划，未改动任何代码。
> 起点：用户提出的第一个方向——**把总结出来的经验 / 知识反哺到项目的 AGENTS 文件中**。
> 状态更新见 §8（2026-10-01：决策已确认，v0.1.1 前置项进行中）。

---

## 0. 结论先行

1. **"反哺 AGENTS"是对的，而且是 MemHub 缺失的最后一环。** 现在的闭环停在"采集 → 总结 → 存成知识笔记"，
   知识笔记只有在用户主动让 Agent 去 `memory_search` 时才会被用上；写进 Agent 每次会话自动加载的指令文件，才是真正的"复利"。
2. **不要一上来就"自动改写用户的 AGENTS.md"。** 这是把"Agent 自己写的、未经验证的记忆"提升为"最高优先级指令"，
   风险（投毒、膨胀、过期、回环）远大于普通读写。推荐拆成两步：
   - **M1a 拉取式（零写入）**：`memhub context` CLI + MCP `rules_get`，由 Agent 按需拉取"已批准的规则集"；
   - **M1b 推送式（受控写回）**：受管块（managed block）+ diff 预览 + 备份 + 一键回滚，默认人工批准。
3. **先补两个前置条件再做写回：** ① `memhub serve` 目前没有 Origin / Host / token 校验，一旦 HTTP 里有"写用户文件"的接口就是 CSRF / DNS rebinding 目标；
   ② 适配器从未在真实目录上跑过（只验证了 demo 数据），要先做真实环境验收。
4. **2026-09-18 的一个新事实改变了目标文件的选择：** Claude Code 2.1.277 起，在没有 `CLAUDE.md` 时会回落读取 `AGENTS.md`。
   因此 **`AGENTS.md` 可以作为跨 Agent 的唯一规范来源**，Claude Code 用一行 `@AGENTS.md` 导入即可（详见 §2）。

---

## 1. 现状盘点（代码级）

| 能力 | 现状 | 位置 |
|---|---|---|
| 采集项目指令文件 | 已支持 `CLAUDE.md` / `AGENTS.md`（含嵌套）/ `GEMINI.md` / `.cursor/rules` / copilot / windsurf / kiro / cline，`kind = instruction` | `crates/memhub-core/src/adapters/projects.rs` |
| 知识笔记落盘 | `Hub::save_knowledge` → `vault/knowledge/<template>/…md`，frontmatter 带 `sources` / `task` / `template`，并重写 `INDEX.md` | `crates/memhub-core/src/hub.rs` |
| 总结任务 | 5 个模板（lessons / preferences / project-brief / dedupe / digest），`tasks::accept` 采纳为知识笔记 | `crates/memhub-core/src/tasks.rs` |
| MCP 工具 | 7 个：`memory_search/read/list/write`、`knowledge_save`、`summary_task_get/submit` | `crates/memhub-core/src/mcp.rs` |
| 写回源文件 | **没有**（设计上镜像是单向、源文件只读；README Roadmap 里有"Write-back with diff preview"） | — |
| HTTP 服务鉴权 | **没有** Origin / Host / token 校验，默认只绑 127.0.0.1 | `crates/memhub-cli/src/serve.rs` |
| 测试 | 6 个 `#[test]`，无适配器夹具测试，CI 无 macOS | `crates/` |
| 真实环境验证 | 只在 `demo/home` 上验证；真实 `~/.claude`、`~/.codex` 等从未扫描过 | — |

一个关键的已有优势：**`projects.rs` 已经把各项目的 `AGENTS.md` 镜像进 vault**，所以"现有指令内容"天然可用于去重与冲突检测，不需要另建读取通道。

---

## 2. 各 Agent 如何读取指令文件（决定写回目标）

> 信息来自 2026 年 5–9 月的公开文章与文档转述，标注"较高 / 中"为我对可信度的判断；落地前应在各 Agent 最新版本上各做一次实测。

| Agent | 全局文件 | 项目文件 | 关键行为 | 可信度 |
|---|---|---|---|---|
| **Codex CLI** | `~/.codex/AGENTS.override.md`，否则 `~/.codex/AGENTS.md` | 从 git 根走到当前目录，每层取 `AGENTS.override.md` > `AGENTS.md` > `project_doc_fallback_filenames` | 每目录最多一个文件；总量上限 `project_doc_max_bytes`（默认 **32 KiB**），超出**静默截断**；override **替换**同层 AGENTS.md | 较高 |
| **Claude Code** | `~/.claude/CLAUDE.md` | `CLAUDE.md`、`.claude/CLAUDE.md`、`CLAUDE.local.md`、`.claude/rules/*.md` | 支持 `@path` 导入（最多 4 层）；**2.1.277（2026-09-18）起**：目录里没有 CLAUDE.md / CLAUDE.local.md 时回落读 `AGENTS.md`；两者并存时只读 CLAUDE.md（除非导入或在 `/config` 改为 `claude-md-and-agents-md`）；`~/.claude/CLAUDE.md` 与 `.claude/rules/` **不计入**"已有 CLAUDE.md"判断；Bedrock / Vertex / Foundry 暂不支持回落 | 较高（有多个来源，且官方文档尚未同步） |
| **Gemini CLI** | `~/.gemini/GEMINI.md` | `GEMINI.md`（向上到 git 根、向下到子目录，全部拼接） | 支持 `@file.md` 导入；默认**不读** AGENTS.md，需在 `settings.json` 设 `context.fileName: ["AGENTS.md","GEMINI.md"]`（个别来源称已原生读取，需实测） | 中 |
| **Cursor** | （用户规则在 GUI 内） | `AGENTS.md`（根与子目录）、`.cursor/rules/*.mdc`（frontmatter：`description` / `globs` / `alwaysApply`）、旧 `.cursorrules` | AGENTS.md 原生读取 | 中 |
| **GitHub Copilot** | — | `.github/copilot-instructions.md`、`.github/instructions/*.instructions.md`（`applyTo`）、`AGENTS.md`（就近优先） | 同上 | 中 |
| **OpenClaw** | `~/.openclaw/workspace/{AGENTS.md,MEMORY.md,USER.md,…}` | 同 workspace | 自有 `memory_search`；`MEMORY.md` 是它自己维护的长期记忆 | 较高（前期已查） |

**由此得到的写回原则：**

- **规范来源 = `AGENTS.md`**（项目级）；Claude Code 侧只在 `CLAUDE.md` 首行加 `@AGENTS.md`（若用户已有 CLAUDE.md，则提示而非强改）。
- **不要写进 Agent 自己维护的记忆**（Codex `~/.codex/memories/`、Claude `…/memory/MEMORY.md`、OpenClaw `MEMORY.md`）——它们有自己的整理机制（Auto Dream / consolidation），手改不受支持且会被覆盖。写回只针对"用户给 Agent 的指令文件"。
- **避开两个会"静默改变行为"的坑：**
  - `CLAUDE.local.md` 会让 Claude 不再回落读 AGENTS.md → 个人级项目规则改用 `.claude/rules/memhub.md`；
  - Codex 的 `AGENTS.override.md` 会**替换**同层 AGENTS.md → MemHub 永远不创建 override 文件。
- **体积预算**：Codex 链路 32 KiB 是硬上限，Claude 官方建议约 200 行内；MemHub 的受管块默认 **≤ 40 行 / 4 KiB**，超出要求用户取舍，而不是无限追加。

---

## 3. 方向一（推荐主线）：知识回灌 —— "晋升"流水线

### 3.1 产品形态

把"知识笔记"与"会被每次会话加载的规则"区分开：**笔记可以多、可以长、可以不确定；规则必须少、短、可执行、经人确认。**
中间加一道"晋升（promote）"。

```
记忆(mirror/inbox) ──总结──► 知识笔记(knowledge) ──晋升──► 规则(rules, 已批准)
                                                        │
                        ┌───────────────┬───────────────┘
                        ▼               ▼
               拉取：memhub context   推送：写入受管块（diff 预览 + 备份）
               MCP rules_get          AGENTS.md / GEMINI.md / .claude/rules / .mdc
```

### 3.2 数据模型（尽量复用现有 frontmatter）

- 新增 `Kind::Rule`，存放 `vault/rules/<scope>/<slug>.md`（Vault 原生，可编辑、可 git）。scope = `global` 或 `project:<slug>`。
- frontmatter 复用既有字段，并在 `extra` 里加：
  `status: draft|approved|applied|retired`、`verified: <date>`、`expires: <date|none>`、`targets: [...]`；
  `sources: [知识笔记/记忆 id…]` 保证**每条规则可回溯到证据**（这是 MemHub 区别于"手写 AGENTS.md"的核心卖点）。
- 新增模板 `rules.md`（总结任务的一种）：输入 = 选定范围的 lessons / preferences / project-brief + **该项目现有 AGENTS.md 镜像**，
  输出 = 若干条"一行一规则"的草稿，要求：祈使句、可验证、附证据 id、不得与现有指令重复或矛盾（直接复用 `dedupe` 的思路）。
- 新增配置 `[[targets]]`（示例）：

  ```toml
  [[targets]]
  scope  = "project:my-app"
  file   = "~/dev/my-app/AGENTS.md"
  mode   = "block"            # block=受管块；import=独立文件+导入行
  format = "markdown"         # markdown | cursor-mdc | claude-rule
  max_lines = 40

  [[targets]]
  scope  = "global"
  file   = "~/.codex/AGENTS.md"
  mode   = "block"
  ```

### 3.3 写回机制（M1b，最小安全集）

1. **受管块**：只读写 `<!-- memhub:begin v1 hash=… -->` … `<!-- memhub:end -->` 之间的内容，块外一字节不动；文件不存在则创建。
2. **保真**：保留 BOM、CRLF / LF、末尾换行；原子写（临时文件 + rename）。
3. **预览**：UI 里展示统一 diff（前端用 `diff` 库即可，Rust 侧不必加依赖）；CLI 提供 `memhub apply --dry-run`。
4. **备份与回滚**：写前复制到 `~/.memhub/backups/<时间>/<相对路径>`；`memhub rollback <apply-id>`；目标在 git 仓库内时只提示、**不替用户 commit**。
5. **漂移检测**：账本 `~/.memhub/applied.json` 记录每次写入的块哈希；下次写入发现块被手改 → 三选一：保留手改 / 覆盖 / 合并为新草稿。
6. **`import` 模式**（更温和）：把规则写到独立文件（如 `.memhub/rules.md`），目标文件只加一行引用——Claude / Gemini 用 `@.memhub/rules.md`，AGENTS.md 无导入语法，只能用 block 模式或在文字中指路。

### 3.4 必须正面处理的风险

| 风险 | 说明 | 对策 |
|---|---|---|
| **回环污染** | 写进 AGENTS.md 的受管块会被 `projects` 适配器再次镜像 → 被下一轮总结当作"新证据" → 自我强化、重复 | 镜像保持原文不变（沿用"镜像 = 源文件原文"约定），但**索引与总结任务上下文构建时剔除受管块区间**；frontmatter 标 `managed_blocks: true` |
| **提示注入 / 记忆投毒** | 记忆来自多个 Agent，含网页抓取内容；晋升为指令等于提权 | 只有 `approved` 才可写；默认**永远人工批准**；写前复用 `redact` 扫密钥，并加一组规则（`curl … \| sh`、`rm -rf`、"ignore previous instructions"、外链 URL）命中则强提示 |
| **膨胀 / 过期** | 规则只增不减，最终被 32 KiB 截断或淹没关键内容 | 预算上限 + 按"置信度、最近验证、被证据引用次数"排序截断；`expires` 到期提示复核；UI 里有"规则健康度"页 |
| **冲突** | 新规则与用户手写规则或其他 Agent 的规则矛盾 | 晋升前跑一次 dedupe / conflict 任务，结果作为审批界面的警告条 |
| **团队仓库被污染** | 项目级 AGENTS.md 通常已提交，受管块会进入同事的 diff | 首次写入前明确提示"此文件被 git 跟踪"；提供 `mode=import` + `.git/info/exclude` 的个人方案；默认不 commit |
| **HTTP 写接口被网页滥用** | `serve` 无鉴权，浏览器里的任意网页可对 `127.0.0.1` 发 POST；桌面 sidecar 亦同 | 写回相关接口**强制**：随机 token（启动时生成，Tauri 通过 IPC 注入）+ 校验 `Host` / `Origin`；否则只允许 Tauri IPC 与 CLI 触发 |
| **各 Agent 行为随版本漂移** | 如 2026-09 的 AGENTS.md 回落，官方文档都还没跟上 | 目标能力写成数据表（`targets.toml`），带"最后实测版本"；`memhub doctor` 用各 Agent 自带的"总结当前指令"提示语做加载验证（Codex 文档推荐此法） |

### 3.5 M1a：拉取式通道（建议先做，工作量小、无写文件风险）

- MCP 新工具 `rules_get(project?, format?)`：返回已批准规则的紧凑 Markdown；`initialize.instructions` 里加一句"开始任务前调用"。
- CLI：`memhub context --project <path> [--format md|json]`，输出可被 hooks / 脚本消费（Claude Code 的 SessionStart 类 hook 可直接注入，具体配置语法落地时对照官方文档核实）。
- 对 OpenClaw 这类自带检索的 Agent，直接暴露 `memory_search` 已够用。
- 价值：即便用户拒绝任何写回，仍能获得"跨 Agent 共享规则"的收益；同时这是对规则内容质量的低成本试运行。

### 3.6 里程碑与验收

| 里程碑 | 内容 | 验收 |
|---|---|---|
| **M0.5 前置** | ① 真实目录扫描验收 + 适配器夹具测试 ② `serve` 加 token / Origin / Host 校验 ③ CI 加 macOS | 真实机器 `detect/scan` 无误报；新增夹具测试 ≥ 12 个；跨站 POST 被 403 |
| **M1a 拉取** | `Kind::Rule`、`rules` 模板、规则审批 UI（草稿 → 批准 → 退役）、`rules_get`、`memhub context` | 在 Claude Code 与 Codex 里各跑一次：新会话能引用到已批准规则 |
| **M1b 推送** | `[[targets]]`、受管块写入器、diff 预览、备份 / 回滚、漂移检测、回环剔除 | 往返测试：apply → 手改块外 → 再 apply，块外不变；回滚后字节级一致（sha256） |
| **M1c 体验** | `doctor` 加载验证、规则健康度页、到期复核提醒 | 超预算、过期、冲突三种情形均有可见提示 |

---

## 4. 其他方向与优先级

| 优先级 | 方向 | 为什么 | 粗估 |
|---|---|---|---|
| **P0** | 真实环境验收与测试补强 | 目前所有"已验证"都基于 demo；Claude 项目目录的路径编码（Windows 盘符、中文路径）等最容易在真实数据上出错；6 个测试对公开项目偏少 | 1–2 天 |
| **P0** | `serve` / HTTP 安全加固 | 见 §3.4；也是任何远程 / NAS 用法的前提 | 0.5–1 天 |
| **P1** | 会话转录摘要（`.jsonl` → 每会话 recap） | "经验"真正的原料在会话里，而不是 Agent 主动写下的几行记忆；已在 Roadmap。需增量（按偏移量）、强脱敏、体积控制 | 3–5 天 |
| **P1** | 去重 / 冲突视图 | 晋升审批的前置依赖；`dedupe` 模板已有，缺 UI 与自动触发 | 2–3 天 |
| **P2** | 触发与提醒 | "距上次 lessons 总结已新增 N 条记忆"；托盘 / 概览页提示；可选定时创建任务（仍由用户的 Agent 执行） | 2 天 |
| **P2** | 分发与更新 | macOS / Windows 代码签名、Tauri updater、winget / Homebrew cask；当前未签名会有 SmartScreen / Gatekeeper 警告 | 按账号条件 |
| **P3** | 更多适配器 | Aider、Continue、Roo Code、Cline 全局记忆 | 每个 0.5 天 |
| **P3** | 可选向量检索 | 设计文档里的 v0.3 评估项；FTS5 trigram 对当前规模够用，暂不建议 | — |

---

## 5. 建议路线

```
v0.1.1  稳定性   M0.5 前置（真实验收、测试、serve 加固、CI macOS）
v0.2.0  回灌     M1a 拉取式 + 规则审批 UI + 去重/冲突视图
v0.3.0  写回     M1b 受管块写入（默认 dry-run + 人工批准）+ doctor
v0.4.0  数据源   会话转录摘要 + 触发提醒
```

排序理由：先让已有功能在真实数据上可信，再让规则"能被用上"（低风险），最后才"改用户文件"（高风险），
同时每一步都能独立发布，失败时可以停在前一步而不留半成品。

---

## 6. 需要你拍板的问题

1. **写回的默认目标**：只写全局文件（`~/.codex/AGENTS.md`、`~/.claude/CLAUDE.md`、`~/.gemini/GEMINI.md`），还是也允许改项目仓库里已提交的 `AGENTS.md`？（建议：项目级默认 `import` 模式，`block` 模式需显式开启。）
2. **是否允许"自动批准"**：建议 v0.3 之前一律人工批准；之后可做"仅限 tag=style 的低风险规则"这类白名单，是否需要？
3. **规则粒度**：按项目为主，还是先做"全局偏好"（个人编码风格、沟通语言）？后者风险小、见效快，适合作为 M1a 的首批试点。
4. **是否先做 M0.5 再做回灌**：我建议先做；如果你更想尽快看到回灌效果，可以把 M1a 与 M0.5 并行，但 serve 加固必须先于任何写文件接口。

---

## 7. 参考来源

- Claude Code 2.1.277 AGENTS.md 回落与 `@AGENTS.md` 导入：
  <https://www.progressiverobot.com/2026/09/18/agents-md-claude-code-project-instructions/>、
  <https://www.eesel.ai/blog/claude-code-agents-md>、
  <https://medium.com/@decoding_ai_by_nureravi/does-claude-code-read-your-agents-md-we-checked-283-repos-in-161-it-still-reads-claude-md-cb6199708fc1>
- Codex 指令发现顺序与 32 KiB 上限：
  <https://codex.danielvaughan.com/2026/03/26/agents-md-advanced-patterns/>、
  <https://www.memorylake.ai/en/blogs/stop-codex-skipping-agents-md-rules>
- Gemini CLI `context.fileName`、各 Agent 文件对照：
  <https://qaskills.sh/blog/gemini-cli-gemini-md-configuration-guide>、
  <https://thepromptshelf.dev/blog/agents-md-vs-claude-md-vs-gemini-md-2026/>、
  <https://getunblocked.com/blog/claude-md-vs-agents-md-vs-cursor-rules/>
- 项目内：`docs/DESIGN.md`（架构与决策）、`docs/mcp-github-publish.md`（发布流程）。

---

## 8. 决策记录与进度

### 8.1 已确认的决策（2026-10-01）

| 问题 | 决定 |
|---|---|
| 写回目标 | **全局与项目均可，做成可选项**（每个目标单独配置；项目级默认 `import` 模式，`block` 模式显式开启） |
| 批准方式 | **人工批准**（v0.3 之前没有自动批准） |
| 第一批规则 | **先做"全局个人偏好"**（编码风格、回复语言等），作为 M1a 的试点 |
| 顺序 | **先做 v0.1.1 稳定性，再做拉取式回灌（v0.2.0）** |
| 流程要求 | 开发过程中同步更新文档，并按步骤提交 git |

### 8.2 M0.5 / v0.1.1 进度

| 项 | 状态 | 说明 |
|---|---|---|
| 真实目录验收 | ✅ | 见 `docs/mcp-real-machine-verification.md`；发现并修复 2 个缺陷，记录 3 个限制 |
| 适配器夹具测试 | ✅ | 6 → 35 个测试 |
| `serve` 加固 | ✅ | Host / Origin / Content-Type 校验 + 非本机监听强制令牌；实机 curl 冒烟 12 项全部符合预期 |
| CI 加 macOS | ✅（待 GitHub 上跑一次确认） | `--locked`、clippy 硬门禁 |
| 发布 v0.1.1 | ⏳ | 推送并确认 CI 后打标签 |

### 8.3 由真实验收新增的待办（并入 §4 优先级）

- **P1** Codex 记忆改存 SQLite（`memories_1.sqlite` / `stage1_outputs`）→ 增加只读 SQLite 适配；本机表为空，需夹具库 + 等真实数据。
- **P2** 来源页显示"已发现 N 个文件"，避免"OpenClaw 已安装 / 已启用但 0 条"的困惑。
- **P2** 识别插件管理的规则目录（如 `~/.claude/rules/<插件>/`），决定是否及如何采集。

### 8.4 v0.2.0（拉取式回灌）的首批范围

1. `Kind::Rule` + `vault/rules/global/*.md`（frontmatter：`status` / `sources` / `verified`）。
2. `rules` 总结模板：输入为 preferences / lessons 知识笔记 + 现有 `~/.claude/CLAUDE.md`、`~/.codex/AGENTS.md` 镜像（去重），输出"一行一条"的偏好草稿。
3. UI：规则页（草稿 → 批准 → 退役），逐条显示证据来源。
4. MCP `rules_get(project?, format?)` 与 CLI `memhub context [--project PATH] [--format md|json]`。
5. 写回（v0.3.0）不在本阶段，但数据模型为 `[[targets]]` 预留。
