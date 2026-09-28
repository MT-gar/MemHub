# MemHub 发布到 GitHub：整理清单与发版流程（2026-09-28）

本文记录 MemHub 首次推送到 `https://github.com/MT-gar/MemHub` 之前做的整理工作、
推送步骤，以及之后如何用 GitHub Actions 发版。项目设计见 `docs/DESIGN.md`，
Windows 实机记录见 `docs/mcp-memhub-bootstrap.md`，变更记录见 `CHANGELOG.md`。

## 1. 推送前的整理

### 1.1 敏感信息检查

| 检查项 | 结果 |
|---|---|
| API Key / Token / 私钥 | 全仓库 grep `sk-`、`ghp_`、`AKIA`、`BEGIN PRIVATE KEY` 等模式：只有 `redact.rs` 单元测试里的字母表占位串（`sk-abcdefghijklmnopqrstuvwxyz1234`）和 AWS 文档示例 Key（`AKIAIOSFODNN7EXAMPLE`），均非真实凭据 |
| 用户名 / 本机路径 | 源码与文档中不含 Windows 用户名；本机仓库路径已从 `docs/mcp-memhub-bootstrap.md` 中去除，示例统一用 `~`、`%USERPROFILE%`、`C:\path\to\MemHub` |
| 演示数据 | `demo/` 由 `scripts/demo.sh` 生成，已在 `.gitignore`，不入库 |
| 构建产物 | `target/`、`node_modules/`、`ui/dist/`、`ui/tsconfig.tsbuildinfo`、sidecar 二进制、Tauri `gen/` 均已忽略 |
| 会话临时文件 | `scratch/`（远程脚本与日志）在提交前删除；`.github/` 之外没有任何 bridge 辅助脚本 |

### 1.2 仓库元数据

- `LICENSE`：MIT，版权行 `2026 MT-gar and MemHub contributors`。
- `Cargo.toml`（workspace 与各 crate）：`repository = "https://github.com/MT-gar/MemHub"`、`license = "MIT"`。
- `.gitattributes` 统一 LF（`*.sh` 强制 LF；`*.png`、`*.ico`、`*.icns` 标为 binary；`docs/images/**` 不产生 diff）。
- `.editorconfig`：UTF-8、LF、2 空格（Rust 4 空格）、行尾去空白。
- `.github/ISSUE_TEMPLATE/`（bug 报告、功能 / 适配器请求表单，`config.yml` 提供 Discussions 链接）、`PULL_REQUEST_TEMPLATE.md`、`CONTRIBUTING.md`、`SECURITY.md`。

### 1.3 README 与配图

- `README.md`（英文）与 `README.zh-CN.md`（中文）内容对齐：横幅、徽章、功能表、截图、Mermaid 架构图、
  支持的 Agent、安装、快速开始、MCP 接入、BYOA 总结流程、Vault 结构、CLI 与 `config.toml` 参考、隐私、开发、路线图。
- 截图位于 `docs/images/`，均为 1360×850 的 Web UI 截图（headless Edge，演示数据），共 13 张：
  `en-*` / `zh-*` × {overview, memories, knowledge, tasks, sources, settings} + `en-overview-light`。
  PNG 已量化为 256 色（每张 29–52 KB，总计约 0.5 MB），避免仓库体积膨胀。
- `docs/images/banner.png`（1600×420）由 Logo + 标题 + 中英标语生成。
- 截图之前顺手修掉的 UI 问题：日期按浏览器区域格式化导致英文界面出现中文日期（改为固定 `YYYY-MM-DD HH:mm`）；
  任务页命令片段在 Windows 路径上用 `/` 拼接且只有 bash 形式（新增 Windows 检测与 PowerShell 形式）；
  `project_path` 字段名未本地化。

### 1.4 CI / Release 工作流

| 工作流 | 触发 | 内容 |
|---|---|---|
| `ci.yml` | push / PR 到 `main` | `build-test` 矩阵（ubuntu-latest、windows-latest）：`npm ci && npm run build`（ui）、`cargo build`、`cargo test --workspace`、Linux 上 `cargo clippy`（允许失败）；`desktop`（ubuntu-22.04）：安装 webkit2gtk 等依赖后对 Tauri 壳 `cargo check` |
| `release.yml` | 推送 `v*` 标签 | 四个目标（macOS arm64 / x64、Windows x64 MSVC、Linux x64）：编译 CLI → 复制为 sidecar → 打独立 CLI 压缩包 → `tauri-action` 构建安装包并创建**草稿** Release → 上传 CLI 压缩包 |

## 2. 推送步骤（本机 Git Bash）

```bash
cd <仓库目录>
rm -rf scratch                      # 会话临时文件
git status --short                  # 确认没有辅助脚本 / 构建产物
git add -A
git commit -m "Prepare for GitHub: illustrated README, theme, deep links, Windows task commands, CI"
git remote add origin https://github.com/MT-gar/MemHub.git
git push -u origin main
```

首次推送到 HTTPS 远程时，Git Credential Manager 会弹出浏览器 / 窗口要求登录 GitHub，完成一次后凭据会被缓存。

推送后检查：

1. `https://github.com/MT-gar/MemHub` 首页渲染 README（横幅、徽章、截图、Mermaid 图）。
2. **Actions** 页签出现 `CI` 运行；两个 job 都应通过。首次运行没有缓存，Windows job 大约需要 10 分钟。
3. **Settings → General → Features** 勾选 Issues；**Settings → Actions → General** 确认
   "Workflow permissions" 为 *Read and write*（`release.yml` 需要创建 Release）。
4. 可选：在仓库 About 中填写简介、主题标签（`mcp`、`ai-agents`、`memory`、`tauri`、`rust`、`claude-code`、`codex`、`local-first`）。

## 3. 发版流程

```bash
# 1. 更新 CHANGELOG.md：把 [Unreleased] 改为 [0.1.0] — <日期>
# 2. 确认版本号一致：
#    Cargo.toml (workspace.package.version) · apps/desktop/src-tauri/tauri.conf.json (version)
#    apps/desktop/src-tauri/Cargo.toml · apps/desktop/package.json
git commit -am "Release 0.1.0"
git tag v0.1.0
git push origin main --tags
```

- `release.yml` 会在约 15–25 分钟内完成四个平台的构建，产物挂在一个**草稿** Release 上：
  `MemHub_0.1.0_x64_en-US.msi`、`MemHub_0.1.0_x64-setup.exe`、`MemHub_0.1.0_aarch64.dmg`、`MemHub_0.1.0_x64.dmg`、
  `MemHub_0.1.0_amd64.AppImage`、`MemHub_0.1.0_amd64.deb`，以及 `memhub-cli-<target>.zip|tar.gz`。
- 在 GitHub 上打开草稿 Release，补充说明（可直接粘贴 CHANGELOG 对应段落），点击 **Publish release**。
- macOS 安装包未签名公证，Release 说明中已注明「右键 → 打开」。

如需重新发同一版本：删除草稿 Release 与标签（`git push origin :refs/tags/v0.1.0 && git tag -d v0.1.0`），修复后重新打标签。

## 4. 已知限制 / 后续

- CI 与 Release 工作流在推送前未在真实 GitHub 上运行过；首次运行如失败，优先检查 Tauri 依赖安装步骤和
  `tauri-action` 的 `projectPath` / `args`。
- Release 未做代码签名（Windows SmartScreen 会提示「未知发布者」，macOS 需右键打开）。
- 桌面版自动更新（`tauri-plugin-updater`）尚未接入，见 `docs/DESIGN.md` 路线图。
