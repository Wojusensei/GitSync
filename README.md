# Git Tool

交互式 Git 历史浏览器，可视化提交图、分支、差异对比、Blame 等，帮助开发者更直观地理解和管理 Git 仓库。

🌐 支持 macOS 和 Windows，基于 Rust + Tauri 构建，性能爆炸！

## ✨ 功能

- **提交历史浏览**：交互式提交图，可拖拽排序、悬停高亮
- **分支可视化**：展示所有分支，一键切换
- **提交详情面板**：选中提交查看 Diff、文件变更、统计
- **智能 Blame**：逐行显示代码作者和时间
- **文件时间线**：查看单个文件的完整修改历史
- **仓库健康报告**：扫描大文件、废弃分支、冲突文件
- **贡献者统计**：按提交数和代码行数可视化
- **热点文件分析**：列出修改最频繁的 Top 20 文件
- **Stash 管理**：保存、查看、弹出、删除 Stash
- **交互式 Rebase**：拖拽挑选提交进行变基操作
- **语义代码搜索**：搜索代码内容在所有历史变更中的出现
- **差异对比**：任意两个提交并排对比
- **命令面板**：按 `Cmd/Ctrl + K` 快速访问所有功能
- **图形化冲突解决**：三列并排对比，一键采用当前/传入分支
- **插件脚本系统**：自定义脚本扩展，放入 `~/.git-tool/scripts/` 即可用
- **折叠面板**：侧边栏功能分组，按需展开

## 📥 下载

前往 [Releases](https://github.com/Wojusensei/git-tool/releases) 页面获取最新安装包。

### macOS

- [git-tool_0.2.0_aarch64.dmg](https://github.com/Wojusensei/git-tool/releases/download/v0.2.0/git-tool_0.2.0_aarch64.dmg)

双击 `.dmg` 文件，将 Git Tool 拖入 **Applications** 文件夹即可完成安装。

### Windows

- [git-tool_0.2.0_x64-setup.exe](https://github.com/Wojusensei/git-tool/releases/download/v0.2.0/git-tool_0.2.0_x64-setup.exe)

双击 `.exe` 安装程序，按提示完成安装。

## 🚀 使用

1. 启动 Git-Tool
2. 在顶部输入框中输入本地 Git 仓库的完整路径（例如 `/Users/name/projects/my-repo`）
3. 按回车或点击 **加载** 按钮
4. 主区域将显示提交历史，侧边栏提供分支、分析工具等功能入口
5. 点击提交可查看详情，点击文件旁的按钮可查看 Blame 或时间线
6. 按 `Cmd/Ctrl + K` 打开命令面板，可快速调用任何功能

## 🛠 技术栈

- **后端**：Rust (git2, reqwest, serde)
- **前端**：TypeScript + React + Framer Motion
- **桌面框架**：Tauri
- **跨平台构建**：macOS 原生编译，Windows 通过交叉编译生成


## 🤝 贡献

欢迎提交 Issue 和 Pull Request。请确保代码符合项目风格，并在提交前运行 `cargo clippy` 和 `npm run build` 检查。欢迎所有用户开发插件，未来或将对插件进行统一管理

## 📄 开源协议

♿️说的道理♿️学院
