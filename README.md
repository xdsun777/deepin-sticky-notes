# Deepin 极简便签 (deepin-sticky-notes)

> 轻量、低打扰的 Deepin 桌面便签应用 · Rust + Slint

一款面向 **Deepin Linux V23** 的桌面便签：多便签窗口、文本与待办混合编辑、Markdown 渲染、置顶、系统托盘与全局热键，配色完全跟随系统主题（也可手动切换）。

---

## 目录

1. [功能特性](#功能特性)
2. [技术栈](#技术栈)
3. [项目结构](#项目结构)
4. [快速开始](#快速开始)
5. [打包为 .deb](#打包为-deb)
6. [使用说明](#使用说明)
7. [架构与实现](#架构与实现)
8. [数据存储](#数据存储)
9. [已知限制](#已知限制)
10. [许可证](#许可证)

---

## 功能特性

- **多便签窗口**：无边框窗口，拖拽移动、边缘缩放（最小 200×200）、置顶、圆角 + 投影。
- **文本与待办混合编辑**：文本块（多行段落）与待办块（复选框）混排。
- **待办交互**：勾选删除线 + 淡入对勾、退格降级、左侧手柄拖拽排序、Shift 多选删除。
- **Markdown 渲染**：加粗、斜体、删除线、行内代码、链接、无序/有序列表（见[已知限制](#已知限制)）。
- **系统托盘**：左键单击切换显示/隐藏、双击新建、右键菜单（新建/显示全部/隐藏全部/退出）。
- **全局热键**：`Ctrl+Alt+N` 新建便签；Wayland 下失效时窗口内同样快捷键兜底。
- **主题**：默认跟随系统亮/暗（`gsettings` 监听，200ms 平滑过渡）；便签右上角 ☀/☾ 手动切换。
- **可靠持久化**：500ms 防抖保存，退出强制落盘；重启恢复位置、大小、置顶与内容。
- **删除便签**：右上角垃圾桶按钮永久删除（区别于 ✕ 关闭即隐藏）。

---

## 技术栈

| 类别 | 选择 |
| --- | --- |
| 语言 | Rust 2021 edition |
| UI | [Slint](https://slint.dev) 1.18（winit + femtovg 后端） |
| 系统托盘 | `tray-icon` 0.25（KSNI / StatusNotifierItem） |
| 全局热键 | `global-hotkey` 0.8 |
| 序列化 | `serde` + `serde_json` |
| 数据路径 | `directories`（XDG Base Directory） |
| 错误处理 | `anyhow` |

---

## 项目结构

```
deepin-sticky-notes/
├── Cargo.toml                 # 依赖 + [package.metadata.deb] 打包配置
├── build.rs                   # 编译 Slint UI
├── src/
│   ├── main.rs                # 入口，初始化事件循环
│   ├── app.rs                 # 全局状态：窗口管理、托盘/热键/主题/防抖编排
│   ├── model.rs               # 数据模型：BlockType / Block / Note / AppState
│   ├── storage.rs             # JSON 读写（XDG 路径）
│   ├── tray.rs                # 托盘图标、菜单与事件
│   ├── theme.rs               # 系统主题监听（gsettings get/monitor）
│   ├── hotkey.rs              # 全局热键注册
│   └── ui/
│       ├── app.slint          # 编译入口，导入并 re-export
│       ├── note_window.slint  # 便签窗口 + 块编辑器
│       └── theme.slint        # 全局色板（亮/暗）
├── resources/
│   ├── icons/512x512.png                  # 应用图标
│   ├── com.deepin.stickynotes.desktop     # 桌面启动项
│   └── com.deepin.stickynotes.autostart.desktop  # 登录自启项
└── docs/
    ├── 01-交互设计文档.md
    ├── 02-UI设计规范.md
    ├── 03-开发实现路径.md
    └── README.md              # 用户操作手册
```

---

## 快速开始

### 环境要求

- Rust 工具链（`rustc` + `cargo`）
- Linux 桌面环境（X11 或 Wayland），OpenGL 支持
- Deepin 下需 `gsettings`（主题跟随）与 DBus（托盘）

### 构建与运行

```bash
git clone <repo-url> && cd deepin-sticky-notes

# 调试运行
cargo run

# 发布构建
cargo build --release
# 产物：target/release/deepin-sticky-notes
```

### 运行测试

```bash
cargo test
```

---

## 打包为 .deb

### 方式一：`cargo deb`（推荐，需 `cargo-deb`）

```bash
cargo install cargo-deb --locked
cargo deb
```

`Cargo.toml` 中的 `[package.metadata.deb]` 已配置好安装路径：可执行文件 → `/usr/bin`、启动项 → `/usr/share/applications`、自启项 → `/etc/xdg/autostart`、图标 → `/usr/share/icons/hicolor`。

### 方式二：手动 `dpkg-deb`（无需额外工具）

```bash
cargo build --release
mkdir -p .deb-stage/DEBIAN .deb-stage/usr/bin \
         .deb-stage/usr/share/applications \
         .deb-stage/usr/share/icons/hicolor/512x512/apps \
         .deb-stage/etc/xdg/autostart

cp target/release/deepin-sticky-notes .deb-stage/usr/bin/
cp resources/com.deepin.stickynotes.desktop .deb-stage/usr/share/applications/
cp resources/com.deepin.stickynotes.autostart.desktop .deb-stage/etc/xdg/autostart/
cp resources/icons/512x512.png .deb-stage/usr/share/icons/hicolor/512x512/apps/deepin-sticky-notes.png

# 编写 .deb-stage/DEBIAN/control（见下方依赖说明）
dpkg-deb --build --root-owner-group .deb-stage deepin-sticky-notes_0.1.0_amd64.deb
```

> 运行时依赖（`Depends`）：`libc6`、`libgcc-s1`、`libfontconfig1`、`libfreetype6`、`libexpat1`、`zlib1g`、`libbz2-1.0`、`libpng16-16`、`libbrotli1`、`libgl1`、`libegl1`、`libxkbcommon0`、`libwayland-client0`、`libx11-6`、`libx11-xcb1`、`libxcb1`。

安装：

```bash
sudo apt install ./deepin-sticky-notes_0.1.0_amd64.deb
# 或
sudo dpkg -i deepin-sticky-notes_0.1.0_amd64.deb
```

安装后从启动器搜索「Deepin 极简便签」启动，登录时自动运行。

---

## 使用说明

完整操作指南见 **[docs/README.md](docs/README.md)**，要点概览：

- **移动 / 缩放**：按住便签空白区域拖拽；从边缘/四角缩放（最小 200×200）。
- **右上角按钮**（从左到右）：☀/☾ 主题切换 → 图钉置顶 → 垃圾桶删除 → ✕ 关闭（隐藏）。
- **编辑**：文本块内回车换行；待办块回车新建文本块；右键菜单「插入待办项」或空行输入 `- [ ]`。
- **Markdown**：`**加粗**`、`*斜体*`、`~~删除线~~`、`` `代码` ``、`[文字](url)`、`- 列表`。
- **托盘**：左键单击切换显示/隐藏，双击新建，右键菜单含退出。
- **新建便签**：`Ctrl+Alt+N` / 托盘双击 / 托盘右键菜单。

---

## 架构与实现

核心设计：**Slint 运行时模型是唯一的运行时数据源**，Rust 负责编排与持久化。

- `App`（`src/app.rs`）持有 `HashMap<String, NoteHandle>`，每个便签窗口对应一个 `Rc<VecModel<BlockItem>>` 块模型。
- 内容编辑通过 `text <=> content` 双向绑定直接写入模型；结构性操作（插入/删除/转换/排序）由 Rust 回调修改模型；每次编辑触发 `render_markdown` 重新解析 Markdown 存入 `rendered` 字段。
- 持久化时由 Rust 将运行时模型快照为 `AppState`（`Note`/`Block`）序列化到 JSON。
- 托盘 / 热键 / 主题变更通过一个 120ms 轮询计时器统一分发（`gsettings monitor` 后台线程 + 轮询兜底）。
- 主题以 `export global Theme` 暴露色板，`App` 在系统切换或手动切换时对所有窗口 `set_dark_mode`。

数据模型（`src/model.rs`）与 `docs/03-开发实现路径.md` 一致：

```rust
enum BlockType { Text, Todo }
struct Block { id, block_type, content, checked: Option<bool> }
struct Note  { id, position: (i32, i32), size: (u32, u32), always_on_top: bool, blocks: Vec<Block> }
struct AppState { notes: Vec<Note> }
```

---

## 数据存储

- 路径：`~/.local/share/deepin-sticky-notes/notes.json`（XDG Data 目录）。
- 时机：内容 / 置顶 / 尺寸 / 位置变化后 500ms 防抖；退出时强制 flush。
- 内容：位置、大小、置顶状态、全部文本与待办块（含 `- [ ]` / `- [x]` 解析）。

---

## 已知限制

| 项 | 说明 |
| --- | --- |
| 块级 Markdown | `# 标题`、代码块、表格、引用、图片、任务列表 `- [ ]` 不渲染（Slint `StyledText` 的 CommonMark 子集所限），会回退为原文 |
| 待办拖拽排序 | 按 32px/行估算目标，含多行块时略有偏差 |
| `- [ ]` 自动转换 | 仅当整个文本块内容恰好为 `- [ ]` 时触发；多行块内请用右键菜单 |
| 纯 Wayland（无 XWayland） | 新建便签「屏幕中心偏右」退化为级联；全局热键可能失效（窗口内 `Ctrl+Alt+N` 兜底） |
| 托盘双击 | `tray-icon` KSNI 后端不产生双击事件，由 350ms 窗口手动判定 |
| 窗口投影 | 依赖 femtovg 渲染器；软件渲染器下不显示 |

---

## 许可证

[MIT](LICENSE)
