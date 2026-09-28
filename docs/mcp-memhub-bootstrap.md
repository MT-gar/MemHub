# MemHub 本地落地记录（ShunCode Bridge 会话，2026-09-28）

本文档记录 MemHub M0 脚手架从云端沙箱搬到本机 Windows 仓库目录的过程、
在 Windows 上的验证结果、为此做的代码修正，以及后续如何运行和接入真实 Agent。
项目总体设计见 `docs/DESIGN.md`；变更记录见 `CHANGELOG.md`。

## 1. 落地内容

| 项目 | 说明 |
|---|---|
| 传输方式 | 文本文件经 MCP `apply_patch`（`*** Add File`）分 5 批写入；`apps/desktop/logo-source.png`（1.2 MB）以 base64 分 7 段写入后 `base64 -d` 还原 |
| 完整性 | 76 个文本文件 + Logo 逐一 `sha256sum` 比对，与沙箱版本全部一致 |
| 未搬运 | `target/`、`node_modules/`、`ui/dist/`、`demo/`（均为构建产物或演示数据，本机重新生成）；Tauri 图标改为本机 `npm run icons --prefix apps/desktop` 重新生成 |
| 新增文件 | `.gitattributes`（统一 LF）、本文档 |

## 2. 本机环境与关键决定

- Windows 10.0.26200，git 2.55，node 24.16，npm 11.17，rustc/cargo 1.93.0。
- rustup 默认工具链原为 `1.93.0-x86_64-pc-windows-gnu`，且本机没有 MinGW；编译 `windows-sys` 时报
  `error calling dlltool 'dlltool.exe': program not found`。
- 本机同时装有 `1.93.0-x86_64-pc-windows-msvc`、Visual Studio 18（Community + BuildTools，MSVC 14.51）
  和 Windows SDK 10.0.26100/28000，因此在仓库目录执行：

  ```bash
  rustup override set 1.93.0-x86_64-pc-windows-msvc
  ```

  该 override 记录在 rustup 本机设置里（不进仓库）；Tauri 在 Windows 上本来就要求 MSVC。

## 3. Windows 验证结果

| 步骤 | 命令 | 结果 |
|---|---|---|
| Web UI 构建 | `cd ui && npm ci && npm run build` | 通过（`tsc -b && vite build`，346 KB JS） |
| Tauri 图标 | `cd apps/desktop && npm ci && npm run icons` | 通过，生成 `src-tauri/icons/*`（已删除 android/ios 子目录） |
| Rust 编译 | `cargo build` | 通过，零警告 |
| 单元/集成测试 | `cargo test` | 6 passed（frontmatter ×2、redact ×2、端到端 generic source、MCP 握手） |
| 演示数据 | `bash scripts/demo.sh` + `memhub detect/scan/list/search` | 识别 4 个已安装 Agent；同步 21 条（demo 中全部 21 个记忆/指令文件） |
| 浏览器模式 | `memhub serve --port 7337` + curl | `/api/health`、`get_overview`、`search_entries`、首页 HTML（rust-embed）均正常 |
| BYOA 闭环 | `create_task` → `get_task` → `submit_task_result` → `accept_task` | 通过；生成 `vault/knowledge/lessons/2026-09-28-经验教训-windows-验证-M807.md` 与 `INDEX.md` |
| MCP stdio | `memhub mcp --agent probe`（initialize + `memory_search`） | 通过，返回正确结果 |
| CLI sidecar | `bash scripts/build-sidecar.sh` | 通过，`apps/desktop/src-tauri/binaries/memhub-x86_64-pc-windows-msvc.exe`（5.7 MB） |
| Tauri 壳类型检查 | `cd apps/desktop/src-tauri && cargo check` | 通过（沙箱中因缺 webkit2gtk 无法验证的部分，本机首次验证） |
| Tauri 打包 | `cd apps/desktop && npm run build` | 见第 6 节 |

## 4. 为 Windows 做的代码修正（已同步回沙箱版本）

| 文件 | 修正 |
|---|---|
| `crates/memhub-core/src/config.rs` | 新增 `MEMHUB_USER_HOME`：覆盖适配器扫描的用户主目录。Windows 下 `dirs::home_dir()` 忽略 `HOME`，否则 demo/测试无法指向假主目录。新增 `native()`：把来自环境变量/配置的 `/` 分隔路径统一为平台分隔符，避免出现 `C:/a/b\c` 混用 |
| `crates/memhub-core/src/adapters/util.rs` | `home()` 改为走 `config::user_home()` |
| `crates/memhub-core/src/watch.rs` | 过滤 `.git` 目录事件前先把 `\` 归一为 `/` |
| `scripts/demo.sh` | 配置中的项目路径用 `cygpath -m` 转成 `C:/...`（Git Bash 下 `/c/...` 对 Windows 程序不可见）；示例命令加上 `MEMHUB_USER_HOME` |
| `scripts/dev.sh` | 同上，导出 `MEMHUB_USER_HOME` |
| `apps/desktop/src-tauri/tauri.conf.json` | `beforeDevCommand` / `beforeBuildCommand` 的相对路径由 `../../../ui` 改为 `../../ui`（Tauri 以 `apps/desktop` 为工作目录执行这两个命令，原路径会让 `tauri build` 失败） |
| `README.md` / `README.zh-CN.md` / `CHANGELOG.md` | 补充 Windows/MSVC 说明与 `MEMHUB_USER_HOME` |

## 5. 本机如何运行

以下命令在 Git Bash 中执行（仓库根目录）。PowerShell 用户把 `export A=B` 换成 `$env:A="B"`，
把 `./target/debug/memhub.exe` 换成 `.\target\debug\memhub.exe` 即可。

```bash
# 浏览器模式（真实数据，默认数据目录 %USERPROFILE%\.memhub）
cargo build -p memhub-cli
./target/debug/memhub.exe detect          # 只读：看看识别到哪些 Agent 目录
./target/debug/memhub.exe scan            # 首次同步到 ~/.memhub/vault
./target/debug/memhub.exe serve           # http://127.0.0.1:7337

# 演示数据（不碰真实目录）
bash scripts/demo.sh
export MEMHUB_USER_HOME="$PWD/demo/home" MEMHUB_HOME="$PWD/demo/home/.memhub"
./target/debug/memhub.exe serve

# 前端热更新开发
bash scripts/dev.sh                       # API :7337 + Vite :1420

# 桌面版
bash scripts/build-sidecar.sh             # 先把 CLI 打成 sidecar
npm run dev --prefix apps/desktop         # 开发窗口
npm run build --prefix apps/desktop       # 安装包：apps/desktop/src-tauri/target/release/bundle/
```

接入真实 Agent（把 `memhub` 换成本机可执行文件的完整路径，例如
`C:\path\to\MemHub\target\release\memhub.exe`）：

```bash
claude mcp add --scope user memhub -- memhub mcp
codex mcp add memhub -- memhub mcp
gemini mcp add memhub memhub mcp
```

其他支持 MCP 的客户端使用：`{"mcpServers":{"memhub":{"command":"memhub","args":["mcp"]}}}`。
UI 的「设置」页会按当前可执行文件路径生成这些片段。

## 6. Tauri 打包与桌面窗口

`npm run build --prefix apps/desktop`（等价 `tauri build`）在本机执行成功（约 5 分钟，含 UI 构建、
release 编译、WiX 与 NSIS 工具自动下载）：

| 产物 | 路径（相对 `apps/desktop/src-tauri/target/release/`） | 大小 |
|---|---|---|
| 可执行文件 | `memhub-desktop.exe` | — |
| MSI 安装包 | `bundle/msi/MemHub_0.1.0_x64_en-US.msi` | 6.56 MiB |
| NSIS 安装包 | `bundle/nsis/MemHub_0.1.0_x64-setup.exe` | 4.08 MiB |

额外验证：

- `memhub-desktop.exe mcp`（无头模式）完成 MCP 握手并列出 7 个工具，说明桌面二进制可直接充当 MCP Server。
- 以演示数据环境启动 `memhub-desktop.exe`，窗口正常打开（WebView2），页面渲染正确；截图见
  `docs/screenshot-windows-knowledge.png`（「知识」页，显示本次 BYOA 闭环生成的知识条目）。
  截图后进程已关闭。

## 7. 未验证事项 / 后续建议

- 桌面窗口只做了启动 + 单页截图，其余页面的交互（同步、编辑、任务创建/采纳、设置保存）尚待人工走查。
- 真实 Agent 目录尚未扫描（本会话所有验证都用 `demo/home` 假主目录），首次 `memhub detect` / `scan`
  建议由本人执行并查看 `%USERPROFILE%\.memhub\vault`。
- GitHub Actions（`ci.yml` / `release.yml`）尚未在真实仓库上跑过；推送到 GitHub 后先看一次 CI。
- `demo/` 不入库；`target/`、`node_modules/`、`ui/dist/`、sidecar 二进制均已在 `.gitignore`。
  会话中的临时脚本目录 `scratch/` 已在结束前删除。
