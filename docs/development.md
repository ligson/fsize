# 开发与构建

## 环境要求

- Node.js 与 pnpm。
- Rust 工具链。
- Tauri 2 在目标操作系统所需的系统库和构建工具，参见 [Tauri 前置条件](https://v2.tauri.app/start/prerequisites/)。

## 常用命令

应用版本维护在 `package.json`、`src-tauri/Cargo.toml` 与 `src-tauri/tauri.conf.json`，三处必须保持一致。正式发布时同步递增版本，并在 `CHANGELOG.md` 添加以版本号和 `YYYY-MM-DD` 日期命名的条目；每次代码或用户可见行为改动都应有变更记录。

安装依赖：

```bash
pnpm install
```

启动 Tauri 桌面开发模式（会按配置启动 Vite，开发地址为 `http://localhost:1420`）：

```bash
pnpm tauri dev
```

执行前端类型检查并生成生产构建：

```bash
pnpm build
```

构建当前主机平台的 Tauri 应用包：

```bash
pnpm tauri build
```

## GitHub 自动发布

`.github/workflows/release.yml` 在推送 `v主版本.次版本.修订版本` 标签时启动。工作流会先校验标签、`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 和 `CHANGELOG.md` 条目一致，再在 Windows、macOS、Ubuntu 上并行构建 Tauri 安装包，并将构建产物和对应变更说明发布到 GitHub Release。仓库的 Actions 设置需允许工作流使用 `GITHUB_TOKEN` 写入 contents。

发版前，先同步更新三个版本字段，补全 `CHANGELOG.md` 中该版本的日期与内容，并运行 `pnpm install --frozen-lockfile`、`pnpm build` 和适用的 Cargo 检查。pnpm 11 使用 `pnpm-workspace.yaml` 的 `allowBuilds` 显式允许或拒绝依赖的安装脚本；新增需运行安装脚本的依赖时，先审阅再列为 `true`。确认要发布后，提交发布相关改动，创建并推送版本标签，例如：

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json CHANGELOG.md
git commit -m "release: v0.3.0"
git tag -a v0.3.0 -m "fsize v0.3.0"
git push origin HEAD
git push origin v0.3.0
```

示例版本号应替换为本次发布版本；若还有其他发布必需文件，应一并审阅并加入提交。标签推送后，在 GitHub Actions 确认三平台构建完成，并检查 GitHub Releases 页面中的安装包和发布说明。

### macOS 签名与公证

macOS 的“应用已损坏，无法打开”通常是 Gatekeeper 拒绝未签名或未公证的下载包。Release 工作流会读取下列 GitHub Actions Secrets；配置后，`tauri-apps/tauri-action` 会在 macOS runner 上签名并提交 Apple 公证：

| Secret | 内容 |
| --- | --- |
| `APPLE_CERTIFICATE` | Developer ID Application 证书导出的 base64 `.p12` 内容 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` 导出密码 |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: 你的名称 (TEAM_ID)` |
| `APPLE_ID` | Apple Developer 账号邮箱 |
| `APPLE_PASSWORD` | App 专用密码，不是 Apple ID 登录密码 |
| `APPLE_TEAM_ID` | Apple Developer Team ID |

证书需要在 Apple Developer 账号中创建并从 macOS 钥匙串导出；App 专用密码在 Apple ID 账户中创建。不要把证书、密码或 App 专用密码写入仓库文件，全部放在 GitHub 仓库的 Settings → Secrets and variables → Actions 中。配置完成后重新发一个版本标签，旧的未签名包不会自动变成已签名包。

临时打开已有的未签名包时，可以在 Finder 中右键应用选择“打开”并确认；若仍被隔离，可在终端执行 `xattr -dr com.apple.quarantine /Applications/fsize.app`，然后再次打开。这个操作只适用于你确认来源可信的安装包，正式分发应使用签名和公证包。

只启动 Vite 浏览器预览服务：

```bash
pnpm dev
pnpm preview
```

纯浏览器模式下不具备完整的 Tauri 文件系统能力；验证文件选择、系统打开和删除操作时应使用 Tauri 桌面开发模式。

应用启动时通过 GitHub Releases API 检查 `ligson/fsize` 的最新正式版本；设置抽屉中也可以手动检查。发现新版本后，界面只打开对应 Release 下载页，不会自动替换正在运行的应用。真正的后台自动安装需要 Tauri updater 公钥、签名更新包和对应的 GitHub Actions 发布产物，当前版本尚未配置这些密钥。

目录扫描由 Rust 后端在后台线程执行。进程复用一个最多 4 个工作线程的 Rayon 线程池；当一个目录有至少 8 个直接子项时，并行读取文件元数据和递归处理子目录，小目录顺序处理以降低调度开销。文件节点直接通过目录项读取元数据，不再进入递归任务或重新构造路径。扫描期间每 250 毫秒至多发送一次当前目录和条目计数，前端状态栏独立更新耗时。扫描树保留在 Rust 后端会话缓存中，完成时只发送根节点和条目数摘要。前端展开目录时通过 `file_size_children` 每页读取 1000 项，超大目录用“加载更多”继续浏览；搜索通过 `file_search` 在后台线程执行，只返回最多 1000 个结果，避免主线程同步处理全量数据。

前端仅持有已展开的树节点和当前搜索结果。Rust 后端缓存只在本次应用会话中有效，刷新或改变根目录会替换缓存；批量文件系统操作由 Rust commands 执行：删除逐项返回成功与失败；移动优先尝试重命名，跨设备时递归复制后删除源文件，目标同名项目不覆盖。跨设备移动应特别检查目标权限和剩余空间。

主题设置通过顶部工具栏的齿轮按钮打开，四种预设及选择持久化由 `src/App.vue` 管理。滚动条使用 Naive UI 的 `n-scrollbar`，样式变量定义在 `src/assets/app.css` 和主题预设中；新增主题时应同时设置文字、背景、边框、强调色与滚动条颜色。

应用图标统一维护在 `public/app-icon.svg`。修改图标后，从仓库根目录执行 `pnpm tauri icon public/app-icon.svg -o src-tauri/icons`，重新生成 Windows、macOS 和其他平台的图标资源。前端品牌标识直接引用此 SVG。

## 跨平台构建

仓库根目录的 `build.sh` 会构建前端，并尝试通过 `cargo-xwin` 生成 Windows MSVC 和 macOS x86_64 目标。脚本使用 Bash、Rust target 和 `cargo-xwin`，实际能否交叉构建还取决于主机平台、工具链和目标平台依赖；它不是所有平台通用的一键构建入口。

## 修改时的检查点

- 修改 Vue/TypeScript 前端后运行 `pnpm build`，它包含 `vue-tsc --noEmit` 和 Vite 生产构建。
- 修改 Rust 命令或 Tauri 注册时，检查 `src-tauri/src/lib.rs`、前端 `invoke` 名称和事件监听名称是否一致。
- 修改文件遍历逻辑时检查：普通文件、空目录、不可读目录、符号链接和大型目录的行为。
- 修改删除操作时检查文件与目录两种路径，并保留明确的用户确认流程。
- 修改 Tauri 插件、权限或能力声明时检查 `src-tauri/capabilities/default.json` 与 `src-tauri/tauri.conf.json`。

当前 `package.json` 未定义独立测试或 lint 脚本；报告验证结果时应区分实际运行的检查与尚未运行的检查。
