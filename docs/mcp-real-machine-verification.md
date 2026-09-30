# 真实机器验收报告（v0.1.1 稳定性，2026-10-01）

> 目的：此前所有"已验证"都基于 `scripts/demo.sh` 生成的演示数据。本次用一台**真实在用**的 Windows 机器
> （装有 Claude Code、Codex CLI、一个空壳 OpenClaw，无 Gemini / Windsurf）做首次真实目录验收。
> 方法：把 `MEMHUB_HOME` 指到一次性临时目录、沿用真实的 `~/.claude` `~/.codex` `~/.openclaw` 作为源（只读），
> 只输出计数与目录结构，不输出任何记忆内容；验收后临时目录已删除，用户自己的 `~/.memhub` 未被触碰。
> 报告中不含任何真实项目名或路径。

## 1. 结果摘要

| 项 | 结果 |
|---|---|
| `detect` | 识别出 Claude Code / Codex CLI / OpenClaw 为已安装，其余为未安装 ✅ |
| 首次扫描 | 17 条、1.5 s（冷），第二次 17 ms 且全部 `unchanged`（幂等）✅ |
| 分布 | Claude 项目记忆 15 条（5 个项目）、Claude 全局 `CLAUDE.md` 1 条、Codex 全局 `AGENTS.md` 1 条 |
| 发现的问题 | **2 个缺陷已修复**（见 §2），**3 个限制已记录**（见 §3） |
| 修复后复测 | 新建配置默认启用 5 个内置来源；5 个项目的名称与 `project_path` 全部正确还原（含中文、空格、连字符路径）✅ |

## 2. 已修复的缺陷

### 2.1 全新安装时所有来源默认是关闭的

- **现象**：在一个全新的 `MEMHUB_HOME` 里 `memhub detect` 显示 Claude Code 等"installed / disabled"，`memhub scan` 得到 `scanned 0`。
  用户必须先到"来源"页逐个打开开关，概览页才有内容；而 README 写的是"自动发现并镜像"。
- **原因**：`Config::default()` 用 `SourceConfig { r#type, ..Default::default() }` 构造内置来源，
  `#[derive(Default)]` 给 `enabled` 的是 `false`；而 serde 反序列化路径使用 `default_true`，两处不一致。
  demo 脚本和既有配置文件都显式写了 `enabled`，所以之前没有暴露。
- **修复**：`Config::default()` 显式 `enabled: true`（未安装的 Agent 本来就会被跳过）。
  测试 `fresh_config_enables_builtin_sources` 覆盖（含保存 / 读取往返）。
- **连带发现**：修复后旧的端到端测试 `end_to_end_generic_source` 开始扫描**开发者本机的真实目录**（得到 19 条而不是 2 条）。
  说明测试依赖了"内置来源默认关闭"——已改为只保留 generic 来源，使测试对运行环境封闭。

### 2.2 Claude Code 项目目录名解码失真（项目被命名为 `C`、`C-2026`、`skill`）

- **背景**：`~/.claude/projects/<编码后的项目路径>/`，编码规则是"**每个非 `[A-Za-z0-9]` 字符都变成 `-`**"
  （路径分隔符、`:`、`_`、`.`、空格，以及**每一个中日韩字符**都变成一个 `-`），因此不可逆。
- **现象**：有记忆但没有会话转录（`*.jsonl` 已被清理）的项目无法从 `cwd` 得知真实路径，
  旧的回退逻辑把整个名字按 `-` 拆开取最后一段，得到 `C`、`2026`、`skill` 这类无意义的项目名，
  并且 `project_path` 为空；同一台机器上 5 个带记忆的项目里有 4 个是这种情况。
- **修复**（`adapters/claude_code.rs`）：
  1. 优先读会话转录里的 `cwd`（原有逻辑）；
  2. 否则**沿真实目录树做带回溯的匹配**：对每个子目录名按同样规则编码，与编码串逐段比对
     （Windows 以 `X--` 开头视为盘符根，POSIX 以 `-` 开头视为 `/`），能精确处理中文、`_`、`.`、空格与连字符；
  3. 项目已不存在时，用可读的 slug 兜底（保留尾部 48 字符；过短则附 6 位哈希，绝不再产生孤立的盘符字母）。
- **测试**：编码规则（含 UTF-16 代理对）、CJK / 下划线 / 空格路径还原、歧义连字符回溯、兜底 slug 的可读性与稳定性、
  同名项目的 slug 去重。
- **兼容性说明**：旧版本已镜像过的 4 个"坏名字"项目，升级后会以新的项目名重新镜像，旧条目被标记为归档（`archived`），不会被删除。

## 3. 已记录、未在 v0.1.1 处理的限制

| 限制 | 详情 | 计划 |
|---|---|---|
| **Codex 记忆存于 SQLite** | 该版本 Codex 有 `~/.codex/memories_1.sqlite`（表 `stage1_outputs`：`raw_memory`、`rollout_summary`、`rollout_slug`、`thread_id`、`generated_at`、`usage_count`…）而**没有** `~/.codex/memories/` 目录；本机表内 0 行，无法在真实数据上验证列含义。当前适配器只读 Markdown，所以读不到 | 下一轮：用只读方式打开 SQLite（`rusqlite` 已内置）导出 `stage1_outputs` 为 `session-summary` / `memory`，先用夹具库写测试，再等真实数据验证 |
| **`~/.claude/rules/**` 未采集** | 本机该目录有 104 个文件，全部来自某个第三方插件（`rules/<plugin>/<语言>/…`）。默认采集会用插件规则淹没用户自己的记忆 | 保持默认不采；需要时可用 `generic` 来源手动添加并用 `exclude` 过滤。后续考虑识别"插件管理目录" |
| **OpenClaw"已安装"但无内容** | 本机 `~/.openclaw` 只有 `state/`，没有 workspace，所以 0 条；`detect` 仍显示 installed（按根目录存在判断） | 来源页增加"已发现 N 个文件"提示，避免"已启用却什么都没有"的困惑 |
| 会话转录（`*.jsonl`）未采集 | 本机 `~/.claude/projects` 下 21 个项目目录里，16 个只有会话转录没有记忆文件 | 见 `docs/mcp-next-steps.md` 的 P1"会话转录摘要" |

## 4. 复现方法（不会触碰 `~/.memhub`）

```bash
export MEMHUB_HOME="$(mktemp -d)"          # 一次性库；源目录仍是真实的 ~/.claude 等（只读）
memhub detect
memhub scan                                 # 期望：scanned N · added N
memhub scan                                 # 期望：unchanged N（幂等）
memhub list --limit 200 | awk '{print $3" | "$4" | "$5}' | sort | uniq -c
rm -rf "$MEMHUB_HOME"
```

Windows 下在 Git Bash 里执行；注意 Git Bash 的 `/tmp` 会被原生程序解释成 `C:\tmp`，建议直接用 `mktemp -d` 的输出并让 `memhub paths` 回显实际位置。
