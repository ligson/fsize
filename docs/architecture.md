# 工程架构与功能上下文

## 项目定位

`fsize` 是一个目录空间查看工具。用户选择一个目录后，应用递归读取文件系统，计算文件大小和目录累计大小，并把结果按层级显示在树中，便于找到占用空间较大的子目录或文件。

桌面原生标题栏承载应用名称和图标；顶部工具栏仅承载空间分析入口、设置与目录选择，避免重复品牌标识。结果标题下方为搜索及批量操作区；输入查询后，搜索结果替代目录树占满列表区域，清空查询恢复树视图。状态栏固定在窗口底部；空闲时显示版本号和主题，扫描时还显示当前处理路径、条目数和耗时。

## 技术组成

- 桌面容器：Tauri 2。
- 前端：Vue 3、TypeScript、Vite、Naive UI。
- 后端：Rust。
- Tauri 插件：Dialog 用于选择目录；Filesystem、Opener 插件已注册；打开路径的命令当前通过 Rust 调用系统文件管理器。
- 前后端交互：前端使用 `invoke` 调用命令；目录扫描和删除在后台线程执行，并由 Tauri 事件反馈结果。

## 主要数据流

1. `src/App.vue` 使用系统目录选择窗口取得路径，并通过 `FileTree` 暴露的 `selectPath` 开始扫描。
2. `src/FileTree.vue` 通过 `invoke('file_size', { path })` 调用 Rust 命令，并监听扫描进度、摘要、失败和删除事件。
3. `src-tauri/src/commands/file_size.rs` 构造目录节点，递归读取子项；文件大小来自目录项元数据，目录大小是其可读取的非符号链接子项的累计值。每层子节点按大小降序排列。进程内复用一个 Rayon 线程池，最多 4 个工作线程；子项达到 8 个时并行处理文件元数据和目录递归，否则顺序处理。完整树保留在后端会话缓存中，扫描完成事件只发送根节点和总数摘要。
4. 前端只先创建根节点；展开目录时通过 `file_size_children` 按 1000 项分页获取子节点，超大目录可逐页继续加载。`file_search` 在 Rust 后台线程查询会话缓存，只将最多 1000 个匹配项返回界面。
5. 右键菜单事件由 `src/TreeNodeContextMenu.vue` 发出，`FileTree.vue` 分发刷新、打开路径和删除操作。

## Tauri 命令与事件

命令在 `src-tauri/src/lib.rs` 注册：

| 命令 | 用途 |
| --- | --- |
| `file_size` | 后台扫描路径并统计空间占用 |
| `file_size_children` | 按路径、偏移和数量分页读取扫描缓存中的目录子项 |
| `file_search` | 后台按文件名或完整路径搜索扫描缓存 |
| `open_folder` | 使用系统默认文件管理器打开路径 |
| `file_delete` | 删除文件，或递归删除目录 |
| `file_delete_many` | 批量删除路径并返回逐项结果 |
| `file_move_many` | 批量移动路径；跨设备时复制后删除源路径，目标同名时跳过并报告失败 |
| `reveal_path` | 在 Windows、macOS 或 Linux 的系统文件管理器中定位路径 |
| `greet` | 模板遗留的问候命令 |

异步事件由前端监听：

| 事件 | 含义 |
| --- | --- |
| `file_size_result` | 扫描成功，payload 是根节点及总条目数/不完整条目数摘要 |
| `file_size_result_error` | 扫描失败 |
| `file_size_progress` | 扫描进度，payload 包含 `path`（当前处理目录）和 `entries_scanned`（已处理条目数） |
| `file_deleted` | 删除成功 |
| `file_deleted_error` | 删除失败 |

状态栏消息通过 `src/component/eventBus.ts` 内的 mitt 事件 `state-message` 在前端组件之间传递。

## 文件职责

| 路径 | 职责 |
| --- | --- |
| `src/App.vue` | 主窗口布局、主题预设与持久化、目录选择和状态栏 |
| `src/FileTree.vue` | 按需分页树节点、异步搜索结果、批量选择、命令调用和操作确认 |
| `src/TreeNodeContextMenu.vue` | 刷新、删除、打开的右键菜单 |
| `src/util/MyTool.ts` | 文件大小显示格式化 |
| `src-tauri/src/lib.rs` | 插件初始化及 Tauri 命令注册 |
| `src-tauri/src/commands/file_size.rs` | 并行目录扫描、内存缓存、分页子项与后台搜索 |
| `src-tauri/src/commands/file_open.rs` | 按操作系统选择 `explorer`、`open` 或 `xdg-open` |
| `src-tauri/src/commands/file_delete.rs` | 文件删除和目录递归删除 |
| `src-tauri/src/commands/file_batch.rs` | 批量删除、跨设备移动与逐项错误结果 |
| `public/app-icon.svg` | 应用品牌与 Tauri 多平台图标的统一源文件 |

## 当前行为边界

- 选择器当前配置为目录选择模式，主要入口是选择要分析的目录。
- 遍历遇到符号链接会跳过，不会跟随链接统计目标内容。
- 遇到无法读取的目录或文件时保留该条目并继续扫描；该条目显示“不可读取”，包含不可读后代的目录显示部分统计值。触发打开或删除等操作时，再由系统权限结果提示失败原因。
- 目录大小是扫描到的文件长度之和，不是磁盘分配空间或压缩前后大小。
- 删除目录使用递归删除，会移除目录及其全部内容；前端在执行前显示确认对话框。
- 扫描期间以节流事件报告当前处理目录和已处理条目数；前端状态栏每秒更新耗时，即使磁盘读取暂时没有新进度时也能看到经过时间。单个条目读取失败不会中止整个扫描。
- Rayon 并行化发生在目录子项处理阶段；进程复用一个最多 4 个工作线程的线程池，为桌面界面留出 CPU 资源。完整扫描树只保存在 Rust 后端，避免序列化并传输百万级 JSON 阻塞 WebView。展开目录按 1000 项分页，单个目录仍可通过“加载更多”继续浏览。
- 四种内置主题（云白、雾青、石墨、深海）保存在 `localStorage` 的 `fsize.theme` 项中。Naive UI 滚动条使用主题变量绘制，WebView 系统滚动条被隐藏。
- 搜索由 Rust 后端遍历当前会话缓存，不是 Lucene 或持久化索引；换目录或刷新后重建缓存。搜索文件名和完整路径，最多显示前 1000 条，避免把百万级搜索索引复制到 WebView。
- Windows 使用 Explorer 的 `/select` 定位文件，macOS 使用 Finder 的 reveal 行为；Linux 打开文件所在目录。批量删除先处理外层目录并跳过已随父目录删除的后代，避免重复报错；批量移动不覆盖目标同名项，也禁止将目录移动到自身或其后代。
- `MyTool.formatSize` 按 1024 进制显示 B 到 YB；调整单位策略时应同步检查零值和单位边界。
- README 和架构文档描述的是当前实现。修改行为后请同步更新，不要把计划中的功能写成已实现功能。
