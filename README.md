# fsize

`fsize` 是一个用于查看磁盘目录占用情况的桌面应用。选择一个目录后，应用会递归统计其中的文件大小，并以树形结构展示目录和文件，帮助快速定位占用空间较大的内容。

当前版本：**0.2.0**。每个版本的功能和修复记录见 [CHANGELOG.md](CHANGELOG.md)。

## 功能

- 通过系统目录选择窗口选择要分析的目录。
- 递归扫描目录，计算文件大小以及各级目录的累计大小。
- 使用可复用的 Rayon 线程池并行扫描较大的目录树，工作线程数限制为 2–8；小目录保持顺序遍历以减少并行调度开销。
- 在树形视图中展示条目名称、类型和大小；每一级的内容按大小从大到小排列。
- 扫描完成后建立本次会话的内存文件索引，可按文件名或完整路径搜索，结果显示所在位置、大小，并支持在系统文件管理器中定位。
- 可在树节点或搜索结果中勾选项目，然后批量删除或移动；删除前确认，移动不覆盖目标目录里的同名项目。
- 遇到无权读取的文件或目录时保留该条目并继续扫描；无法完整统计的目录会标记为部分结果。
- 通过右键菜单刷新扫描结果、在系统文件管理器中打开路径，或删除选中的文件、目录。
- 通过顶部齿轮按钮打开设置，选择两种浅色和两种深色预设主题，选择会保存在本地并在下次启动时恢复。
- 顶部工具栏提供目录选择和设置入口；结果区上方集中放置搜索框与批量操作，搜索结果独占列表区域。
- 文件树使用 Naive UI 自绘滚动条，颜色跟随当前主题；窗口页面不显示系统滚动条。

扫描时会跳过符号链接，以避免重复遍历链接目标。删除目录会递归删除该目录及其所有内容，请在确认后操作。

## 技术架构

应用采用 Tauri 2 桌面架构，前端负责交互和结果展示，Rust 后端负责文件系统操作：

- **前端**：Vue 3、TypeScript、Vite、Naive UI。
- **桌面容器与系统集成**：Tauri 2，以及 Dialog、Filesystem、Opener 插件。
- **后端**：Rust；通过 Tauri Commands 提供目录统计、打开路径和删除路径功能。
- **前后端通信**：前端通过 Tauri `invoke` 调用命令；耗时的目录扫描和删除操作通过 Tauri 事件返回最终结果或错误状态。目录扫描目前没有逐文件进度事件。

## 开发环境

需要安装 Node.js、pnpm、Rust 工具链，以及 Tauri 2 在目标操作系统所需的系统依赖。详细的平台依赖可参考 [Tauri 前置条件](https://v2.tauri.app/start/prerequisites/)。

安装前端依赖：

```bash
pnpm install
```

启动桌面应用开发模式：

```bash
pnpm tauri dev
```

只构建并检查前端：

```bash
pnpm build
```

构建当前平台的桌面安装包：

```bash
pnpm tauri build
```

项目中的 `build.sh` 另外提供了使用 `cargo-xwin` 交叉构建 Windows 和 macOS 目标的命令。跨平台构建需要安装相应的 Rust target 和 `cargo-xwin`，并满足目标平台的构建要求。

## 项目结构

```text
src/                         Vue 前端
  App.vue                    应用布局、主题和目录选择
  FileTree.vue               文件树、操作菜单及前后端交互
  TreeNodeContextMenu.vue    文件树右键菜单
  util/MyTool.ts             文件大小格式化工具
src-tauri/                   Tauri 与 Rust 后端
  src/commands/file_size.rs  递归扫描与目录大小统计
  src/commands/file_open.rs  使用系统文件管理器打开路径
  src/commands/file_delete.rs 删除文件或目录
```

## 开发文档

- [工程架构与功能上下文](docs/architecture.md)
- [开发与构建说明](docs/development.md)
- [仓库协作约定](AGENTS.md)
- [版本更新日志](CHANGELOG.md)

推送与当前版本匹配的 `v*` 标签会触发 GitHub Actions，在 Windows、macOS 和 Ubuntu 上构建安装包并创建 GitHub Release。具体发版步骤见[开发文档](docs/development.md#github-自动发布)。
