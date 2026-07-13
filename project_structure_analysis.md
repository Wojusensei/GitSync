# GitSync 项目结构与核心文件分析

本文档提供了 **GitSync**（一个基于 Tauri + React 的液态玻璃风格 Git 仓库管理工具）的项目文件目录树及每个文件的核心作用说明。

---

## 1. 项目目录结构树

```text
GitSync/
├── .vscode/                     # VS Code 配置文件夹
├── dist/                        # 前端打包输出目录（Build Output）
├── public/                      # 静态资源目录（前端静态图标、资源等）
├── src/                         # 前端源代码目录（React + TypeScript）
│   ├── assets/                  # 前端静态图片/图标等资源
│   ├── components/              # 业务组件库
│   │   ├── ChangelogGenerator.tsx # 变更日志（Changelog）生成组件
│   │   ├── CommandPalette.tsx     # 全局命令/搜索快捷调色板组件
│   │   ├── CommitFilter.tsx       # 提交历史的高级筛选器组件
│   │   ├── ConflictResolver.tsx   # Git 分支合并冲突解决及合并工具组件
│   │   ├── DiffViewer.tsx         # 变更差异（Diff）分析器组件
│   │   ├── EnhancedRebase.tsx     # 交互式变基（Interactive Rebase）界面组件
│   │   ├── ExportHTML.tsx         # HTML 格式分析报告导出组件
│   │   ├── ExportReport.tsx       # Markdown 格式分析报告导出组件
│   │   ├── FileTree.tsx           # 仓库当前版本文件目录树结构组件
│   │   ├── GraphView.tsx          # 提交线索拓扑图（Commit Graph）可视化组件
│   │   ├── HookManager.tsx        # Git Hooks 脚本管理器组件
│   │   ├── MultiRepo.tsx          # 多本地 Git 仓库快速切换与管理组件
│   │   ├── QueryConsole.tsx       # SQL (GitQL) 仓库历史高级检索控制台组件
│   │   ├── RemoteManager.tsx      # Git 远程源（Remote）管理组件
│   │   ├── ScriptRunner.tsx       # 仓库级自定义自动化脚本执行组件
│   │   ├── SemanticSearch.tsx     # 提交历史语义/自然语言检索组件
│   │   ├── SideBySideDiff.tsx     # 左右双栏比照 Diff 对比组件
│   │   ├── SyntaxHighlight.tsx    # 代码差异行语法着色高亮渲染器组件
│   │   ├── TagManager.tsx         # 标签（Tag）查看与快速新建组件
│   │   ├── ThemeToggle.tsx        # 主题管理及快速切换按钮
│   │   ├── TimeMachine.tsx        # 滑块拖动历史快照回溯（时间机器）组件
│   │   └── UIManager.tsx          # UI 配置（背景色、强度、手电筒范围、模式）
│   ├── App.css                  # 全局液态玻璃及组件样式表
│   ├── App.tsx                  # React 顶层核心入口及流程控制中心
│   ├── background.jpg           # 默认液态玻璃底图背景图片
│   ├── bg.txt                   # 背景图片 Base64 数据备份
│   ├── main.tsx                 # React DOM 的挂载入口点
│   └── vite-env.d.ts            # Vite TypeScript 类型声明支持文件
├── src-tauri/                   # Tauri 后端 Rust 源代码目录
│   ├── capabilities/            # 权限集控制配置（v2 规范）
│   ├── gen/                     # Tauri 编译生成的配置文件
│   ├── icons/                   # 跨平台客户端应用程序图标
│   ├── src/
│   │   ├── lib.rs               # 后端通用接口定义层
│   │   └── main.rs              # Tauri 注册与 Git/系统执行业务层（约 1800 行核心）
│   ├── build.rs                 # Cargo 编译脚本文件
│   ├── Cargo.toml               # Rust 包依赖配置文件
│   ├── Cargo.lock               # Rust 依赖锁文件
│   └── tauri.conf.json          # Tauri 应用的核心框架配置文件
├── .gitignore                   # Git 忽略文件配置
├── index.html                   # 前端挂载页面主入口
├── package.json                 # Node.js 项目依赖及运行脚本配置
├── package-lock.json            # npm 依赖项版本锁文件
├── tsconfig.json                # TypeScript 全局编译配置
├── tsconfig.node.json           # Node.js 环境 TypeScript 编译配置
└── vite.config.ts               # Vite 构建及开发服务器配置
```

---

## 2. 核心文件及其作用说明

### 📂 Root 根目录文件
- **[package.json](file:///G:/Code/GitSync/package.json)**：配置 Node 项目依赖（React 19, Tauri API 2.x, Framer Motion 12, TypeScript）与构建脚本。
- **[vite.config.ts](file:///G:/Code/GitSync/vite.config.ts)**：定义 Vite 预构建与编译选项，配置针对 Tauri 容器的本地端口和渲染环境。
- **[index.html](file:///G:/Code/GitSync/index.html)**：Tauri 渲染引擎默认加载的前端静态主入口页面，引入 `src/main.tsx`。
- **[tsconfig.json](file:///G:/Code/GitSync/tsconfig.json)**：指定 TypeScript 的编译目标（ESNext）、模块规范和 strict 类型安全策略。

---

### 📂 后端核心目录 (`src-tauri/`)
- **[tauri.conf.json](file:///G:/Code/GitSync/src-tauri/tauri.conf.json)**：Tauri 的核心配置文件。控制打包参数、应用名、图标路径、后端安全端口以及允许调用的核心 API 权限。
- **[src-tauri/src/main.rs](file:///G:/Code/GitSync/src-tauri/src/main.rs)**：**后端主引擎文件**（核心）。
  - 调用 `git2` (libgit2 绑定) 与本地文件系统。
  - 导出并注册了 `get_commits`, `get_branches`, `add_safe_directory` (Git 安全目录修复), `execute_rebase`, `git_query` (GitQL 分析) 等数十个与 Git 直接通信的系统级 Command API，提供极速响应。

---

### 📂 前端核心页面 (`src/`)
- **[src/main.tsx](file:///G:/Code/GitSync/src/main.tsx)**：前端脚本总挂载入口，负责将 React 的 `<App />` 渲染并绑定至 DOM 中。
- **[src/App.tsx](file:///G:/Code/GitSync/src/App.tsx)**：**前端主控制中心**（核心）。
  - 处理应用程序的全部核心状态（当前仓库路径、提交列表、分支列表、当前选中提交详情、全局 Error 信息等）。
  - 承载整个应用的基本布局结构（左侧功能导航 Sidebar + 顶部仓库输入栏 Topbar + 中央详情数据主视区 Main View）。
  - 内置全局 `error` 监听器，捕获 Git 安全目录缺失错误并渲染 `safe-dir-overlay` 修复窗口。
- **[src/App.css](file:///G:/Code/GitSync/src/App.css)**：**样式核心**。
  - 核心定义了液态玻璃（Liquid Glass）视觉系统（通过 `rgba` 半透明色值配合 `backdrop-filter: blur(...) saturate(...)`）。
  - 引入了鼠标全局跟踪光斑（`.pointer-glow`）与手电筒式 Hover 光效（`.torch-glow`），赋予界面流动感与悬浮光影美学。

---

### 📂 业务组件库 (`src/components/`)
- **[ChangelogGenerator.tsx](file:///G:/Code/GitSync/src/components/ChangelogGenerator.tsx)**：调用后端获取提交信息，过滤合并提交，一键输出富文本日志。
- **[CommandPalette.tsx](file:///G:/Code/GitSync/src/components/CommandPalette.tsx)**：当按下快捷键或触发按钮时，弹出一个磨砂玻璃风快捷命令栏，允许搜索并执行分支切换、健康报告等命令。
- **[CommitFilter.tsx](file:///G:/Code/GitSync/src/components/CommitFilter.tsx)**：过滤特定作者、信息关键字或分支，实时更新中央的提交链。
- **[ConflictResolver.tsx](file:///G:/Code/GitSync/src/components/ConflictResolver.tsx)**：可视化解析 Git 冲突。读取含有冲突标记的文件，并渲染交互界面让用户选择 "使用当前改动"、"使用传入改动" 或合并二者。
- **[DiffViewer.tsx](file:///G:/Code/GitSync/src/components/DiffViewer.tsx)**：展示文件修改的传统补丁（Patch）差异，按行标记增删内容。
- **[EnhancedRebase.tsx](file:///G:/Code/GitSync/src/components/EnhancedRebase.tsx)**：把极难使用的 `git rebase -i` 命令图像化，允许通过拖拽直接调整提交顺序，或一键标记为 `squash`, `reword`, `drop`。
- **[ExportHTML.tsx](file:///G:/Code/GitSync/src/components/ExportHTML.tsx)** & **[ExportReport.tsx](file:///G:/Code/GitSync/src/components/ExportReport.tsx)**：生成仓库概览（包括冲突数、大文件排行、贡献者排行等），导出为网页报告或 Markdown 文本。
- **[FileTree.tsx](file:///G:/Code/GitSync/src/components/FileTree.tsx)**：递归列出当前分支下的全量文件结构，点击可查看单独文件内的 Blame 或历史追踪。
- **[GraphView.tsx](file:///G:/Code/GitSync/src/components/GraphView.tsx)**：画出一个可视化的 SVG 拓扑网络图，清晰呈现分支合并、分叉、HEAD 游移和标签位置。
- **[HookManager.tsx](file:///G:/Code/GitSync/src/components/HookManager.tsx)**：以图形界面读写 `.git/hooks` 下的所有内置脚本，免去终端下对 Hooks 繁琐的操作。
- **[MultiRepo.tsx](file:///G:/Code/GitSync/src/components/MultiRepo.tsx)**：保存最近打开过的仓库历史，以便在多个工程间快速穿梭切换。
- **[QueryConsole.tsx](file:///G:/Code/GitSync/src/components/QueryConsole.tsx)**：自带 GitQL 引擎。输入类似 `SELECT hash, author, message FROM commits LIMIT 10` 的 SQL 指令直接查询仓库信息。
- **[RemoteManager.tsx](file:///G:/Code/GitSync/src/components/RemoteManager.tsx)**：支持添加、编辑或删除 `origin` 等远程地址，方便观察拉取/推送目的地。
- **[ScriptRunner.tsx](file:///G:/Code/GitSync/src/components/ScriptRunner.tsx)**：提供预定义/自定义脚本快捷执行方案，如快速清理冗余垃圾分支或生成开发快照。
- **[SemanticSearch.tsx](file:///G:/Code/GitSync/src/components/SemanticSearch.tsx)**：通过后台轻量模型或者高级特征匹配，让用户用自然语言搜索提交，比单纯 Grep 文本匹配更智能。
- **[SideBySideDiff.tsx](file:///G:/Code/GitSync/src/components/SideBySideDiff.tsx)**：两栏并排展示文件的修改前与修改后，直观找出细节变化。
- **[SyntaxHighlight.tsx](file:///G:/Code/GitSync/src/components/SyntaxHighlight.tsx)**：分析后缀名为 `.ts`, `.rs`, `.py` 等的文件差异，实施高级代码高亮渲染。
- **[TagManager.tsx](file:///G:/Code/GitSync/src/components/TagManager.tsx)**：显示已发布 tag 的发布详情，并提供一键在 HEAD 上贴上语义化版本号。
- **[ThemeToggle.tsx](file:///G:/Code/GitSync/src/components/ThemeToggle.tsx)**：调整应用的色彩主题系统。
- **[TimeMachine.tsx](file:///G:/Code/GitSync/src/components/TimeMachine.tsx)**：拉动进度条即可还原某个瞬间的所有文件内容，类似于 macOS Time Machine，支持一键定位还原。
- **[UIManager.tsx](file:///G:/Code/GitSync/src/components/UIManager.tsx)**：微调液态玻璃视觉特性的后台控制板，支持调整手电筒范围、背景模糊蒙版暗度，以及选择本地图片渲染为毛玻璃底图。
