<div align="center">
  <a href="README_EN.md"><kbd>🌐 English</kbd></a>
</div>

# GitSync 

交互式 Git 历史浏览器，可视化提交图、分支、差异对比、Blame 等，帮助开发者更直观地理解和管理 Git 仓库。

🌐 适配 macOS 和 Windows 以及大部分 Linux 的发行版 ，基于 Rust + Tauri 构建，性能爆炸💥

当前版本：version 0.3.2

## ✨ 功能

### 核心浏览
- **交互式提交历史**：悬停高亮、手电筒效果，配合虚拟滚动支持 1k+ 提交不卡顿
- **分支可视化与管理**：一键切换分支，蓝点跟随当前分支
- **提交详情面板**：查看 Diff、文件变更统计、语法高亮
- **智能 Blame**：逐行显示代码作者和时间
- **文件时间线**：查看单个文件的完整修改历史

### 代码追溯
- **Diff 语法高亮**：添加/删除行用颜色区分，支持了字符级行内高亮
- **并排对比**：任意两个提交的代码并排比较
- **语义代码搜索**：在所有历史变更中搜索代码内容
- **差异对比工具**：快速对比两个提交或分支的差异

### 仓库分析
- **仓库健康报告**：扫描大文件、废弃分支、合并冲突
- **贡献者统计**：按提交数、代码行数可视化
- **热点文件分析**：列出修改最频繁的 Top 20 文件

### 操作增强
- **Stash 管理**：保存、查看、弹出、删除 Stash
- **交互式 Rebase**：拖拽挑选提交进行变基操作
- **标签管理**：创建、查看 Git 标签
- **远程仓库管理**：查看远程地址
- **多仓库管理**：快速切换多个 Git 仓库

### 高级工具
- **图形化合并冲突解决**：三列并排对比，一键采用当前/传入分支
- **插件脚本扩展系统**：自定义脚本放入 `~/.git-tool/scripts/` 即可运行
- **侧边栏折叠面板**：功能分组管理，按需展开
- **Git Hooks 管理器**：浏览、编辑、保存 Git Hook 脚本
- **UI 管理**：深色 / 浅色 / 柔和色模式主题切换，自定义背景，手电筒大小调整等，还可选择MD2

### 搜索与导出
- **提交筛选**：按作者、日期、文件路径过滤
- **导出报告**：生成 Markdown/HTML 分析报告
- **变更日志生成**：根据提交历史自动生成 CHANGELOG

### SQL 微型数据库
- **GitSQL 查询引擎**：用类 SQL 语法查询 Git 仓库
- **多表关联**：支持 `commits` 和 `file_changes` 表的 JOIN
- **聚合查询**：COUNT、SUM、AVG、MAX、MIN + GROUP BY
- **列别名**：`SELECT col AS alias`
- **语法帮助**：内置示例查询、查询历史、CSV 导出
- **友好的错误提示**：表/列不存在时给出明确提示

### UI 体验
- **伪液态玻璃 UI**：毛玻璃效果 + 自定义背景图
- **全局鼠标光斑**：跟随鼠标的柔光效果
- **命令面板**：`Cmd/Ctrl + K` 快速调用所有功能
- **自动滚动**：点击侧边栏自动定位到对应面板，支持叠加/替换两种显示模式

## 📥 下载

前往 [Releases](https://github.com/Wojusensei/GitSync/releases) 页面获取最新安装包。

### macOS

[GitSync_0.3.2_aarch64.dmg](https://github.com/Wojusensei/GitSync/releases/download/v0.3.2/GitSync_0.3.2_aarch64.dmg)

### Windows

[GitSync_0.3.2_x64-setup.exe](https://github.com/Wojusensei/GitSync/releases/download/v0.3.2/GitSync_0.3.2_x64-setup.exe)

### Linux (Debian / Ubuntu)

[GitSync_0.3.2_amd64.deb](https://github.com/Wojusensei/GitSync/releases/download/v0.3.2/GitSync_0.3.2_amd64.deb)

### Linux (Fedora / RHEL)

[GitSync-0.3.2-1.x86_64.rpm](https://github.com/Wojusensei/GitSync/releases/download/v0.3.2/GitSync-0.3.2-1.x86_64.rpm)

### Linux (通用 tar.gz)

[GitSync-0.3.2-linux-x86_64.tar.gz.zip](https://github.com/Wojusensei/GitSync/releases/download/v0.3.2/GitSync-0.3.2-linux-x86_64.tar.gz.zip)

### Arch Linux

[gitsync-0.3.2-1-x86_64.pkg.tar.zst](https://github.com/Wojusensei/GitSync/releases/download/v0.3.2/gitsync-0.3.2-1-x86_64.pkg.tar.zst)

## 😋 安装

### macOS

双击 `.dmg` 文件，将 GitSync 拖入 **Applications** 文件夹。

### Windows

双击 `.exe` 安装程序，按提示完成安装。

### Linux (Debian / Ubuntu)

```bash
sudo dpkg -i GitSync_0.3.2_amd64.deb
```

## Linux (Fedora / RHEL)

```bash
sudo rpm -ivh GitSync-0.3.2-1.x86_64.rpm
```

### Arch Linux

```bash
sudo pacman -U gitsync-0.3.2-1-x86_64.pkg.tar.zst
```

## Linux 通用 tar.gz

```bash
tar -xzvf GitSync-0.3.2-linux-x86_64.tar.gz
./GitSync
```

## 🚀 使用

1. 启动 GitSync
2. 在顶部输入框中输入本地 Git 仓库的完整路径（例如 `/Users/name/projects/my-repo`），或点击 📁 (这玩意在界面偏上的位置)按钮通过对话框选择
3. 按回车或点击 **加载** 按钮
4. 主区域将显示提交历史（默认加载 30 条，滚动到底部自动加载更多）
5. 侧边栏提供分支、分析工具等功能入口
6. 点击提交可查看详情，点击文件旁的按钮可查看 Blame 或时间线
7. 按 `Cmd/Ctrl + K` 打开命令面板，可快速调用任何功能

## 🧪 本地开发与测试

- 开发时先确认当前分支为 `main`，再运行 `npm run tauri dev`。
- 测试分支切换时，不要用 GitSync 打开 GitSync 自己的源码目录。点击分支会把源码仓库切到另一个分支，Tauri 会立即用该分支重新编译，当前运行版本可能变成没有新命令的旧代码。
- 建议准备一个独立测试仓库，例如 `git clone <任意仓库> /tmp/gitsync-test`，再用 GitSync 加载 `/tmp/gitsync-test` 来测试。

## ❓ 常见问题

### 点击分支提示 `Command checkout_branch not found`

这不是当前 `main` 版本的分支切换代码问题，通常说明本地开发版正在运行旧分支或旧构建。最常见原因是上面提到的“用 GitSync 打开了自己的源码目录并切换分支”。这会导致**点击分支后源码仓库被切到旧分支**，此时 Tauri 第一时间用旧分支重新编译，命令从当前运行版本中蒸发导致立即报错。

解决方法：

1. 在 GitSync 源码目录执行 `git checkout main`
2. 重新运行 `npm run tauri dev`
3. 使用独立的测试仓库加载，再验证分支切换

### macOS 提示“已损坏，无法打开”

如果 macOS 提示“GitSync 已损坏，无法打开”，可以先执行以下命令处理：

```bash
sudo xattr -rd com.apple.quarantine /Applications/GitSync.app
codesign --force --deep --sign - /Applications/GitSync.app
open /Applications/GitSync.app
```

未配置 Apple 签名 secrets 时，安装包使用 ad-hoc 签名，打开时可能提示“无法验证开发者”，右键点击图标选择“打开”即可。配置 Apple Developer ID 签名与公证后无需这些操作。

## 🛠 技术栈

- **后端**：Rust (git2, serde, chrono, tauri-plugin-dialog)
- **前端**：TypeScript + React + Framer Motion + react-virtuoso
- **桌面框架**：Tauri v2
- **跨平台构建**：macOS 原生编译，通过交叉编译以及 Github Actions 生成其他操作系统的安装包文件

## 🤝 贡献

欢迎通过 Issue 和 Pull Request 参与 GitSync 的开发。我们欢迎包括修复 bug、增加功能、完善文档、优化 UI 等等在内的一切贡献

请确保代码符合项目风格，并在提交 PR 前运行 `cargo clippy` 和 `npm run build` 检查

**提交 Issue**

提交前请先搜索已有 Issue，避免重复。Issue 请尽量包含以下信息：

- 问题描述：发生了什么，期望结果是什么，可以的话请提供详细日志甚至是录屏资料
- 复现步骤：从启动应用到复现问题的完整操作
- 环境信息：操作系统、GitSync 版本、Node/Rust 版本
- 仓库信息：出问题的 Git 仓库是否公开、仓库规模、是否包含子模块或大文件
- 日志信息：终端输出、错误文案、Git/GitSync 返回的错误信息
- 截图或录屏：能帮助定位 UI 和交互问题


**提交 Pull Request**

- Fork 仓库并在新分支上开发，分支命名建议使用 feat/、fix/、docs/ 前缀，方便分支管理
- 保持改动范围聚焦，单次提交 PR 修复/完善 单个已知问题或关联单个 Issue
- PR 描述中说明动机、改动内容和测试方式
- 如果 PR 修复了某个 Issue，在描述中关联该 Issue

**代码风格**

- Rust 代码遵循 rustfmt 和 clippy
- 前端保持现有 TypeScript + React 风格，尽可能复用现有组件和依赖，不引入新框架；如确有需要，请详细和仓库维护者说明
- UI 文案尽量简洁一致，错误信息使用中文或英文，并与现有文案风格保持一致

**插件脚本**

欢迎贡献插件脚本，自定义脚本放入 `~/.git-tool/scripts/` 即可运行。未来插件数量足够多时会考虑对插件的贡献系统以及插件做统一管理

**Arch Linux**

GitSync 已提供官方 Arch Linux 安装包。如果你希望继续完善 AUR 体验，例如将 PKGBUILD 提交到 AUR、补充更多架构或改进打包脚本，欢迎提交 PR。

## 📄 开源协议

[MIT License](https://opensource.org/licenses/MIT)
