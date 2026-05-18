# 🐱 Aemeath Claude Code Pet — 增强版

> Q 版像素爱弥斯（Aemeath）桌面宠物，与 Claude Code 实时联动。基于 [aemeath_withclaude](https://github.com/77wilNd/aemeath_withclaude) 改造，新增持久化气泡、游戏自动隐身、idle 提醒等功能。

![License](https://img.shields.io/badge/License-MIT-green)
![Platform](https://img.shields.io/badge/Platform-Windows%2010+-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.x-orange)

---

## 目录

- [功能特性](#功能特性)
- [截图预览](#截图预览)
- [前置要求](#前置要求)
- [安装](#安装)
- [功能详解](#功能详解)
- [HTTP API](#http-api)
- [精灵图合并](#精灵图合并)
- [构建](#构建)
- [Token 消耗分析](#token-消耗分析)
- [排错指南](#排错指南)
- [文件清单](#文件清单)
- [致谢](#致谢)

---

## 功能特性

- **15 种像素动画**：idle、running、chatting、fetching、searching、analyzing、building、celebrating、waving、jumping、failed、waiting、review、running-right、running-left
- **持久化气泡**：工具执行期间气泡持续显示，不会 4 秒后消失
- **权限渐进恢复**：等待权限确认时气泡持续显示，15 秒后变淡，60 秒后自动清除
- **Idle 提醒**：空闲 5+ 分钟后每 2-3 分钟随机提醒用户回来工作
- **双击打开 Obsidian**：双击爱弥斯直接打开 Obsidian
- **游戏自动隐身**：检测全屏窗口时自动隐藏，切回桌面自动显示
- **透明悬浮窗**：无边框、可拖拽、始终置顶、不占任务栏
- **系统托盘**：左键切换显隐，右键菜单

---

## 截图预览

| 待机 | 招手 | 执行任务 |
|------|------|----------|
| ![idle](preview/new-idle.gif) | ![waving](preview/new-waving.gif) | ![running](preview/new-running.gif) |

| 跳跃 | 异常 | 完成 |
|------|------|------|
| ![jumping](preview/new-jumping.gif) | ![failed](preview/new-failed.gif) | ![review](preview/new-review.gif) |

| 等待 | Q 版总览 |
|------|----------|
| ![waiting](preview/new-waiting.gif) | ![contact-sheet](preview/contact-sheet.png) |

---

## 前置要求

| 组件 | 版本 | 说明 |
|------|------|------|
| Windows | 10+ | 需要 Win32 API 支持 |
| [Rust](https://rustup.rs/) | stable | 编译 Tauri 后端（仅构建时需要） |
| [Node.js](https://nodejs.org/) | >= 18 | npm 依赖 |
| [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) | 任意 | Tauri 前端渲染引擎（Windows 11 预装） |
| [Python](https://python.org/) | >= 3.10 | 游戏守护脚本 + 精灵图工具 |
| [Pillow](https://python-pillow.org/) | 任意 | 精灵图合并/预览生成（仅构建时需要） |
| [Claude Code](https://claude.ai/code) | 最新 | AI 编码助手 |

---

## 安装

### 1. 下载

从 [Releases](https://github.com/77wilNd/aemeath_withclaude/releases/latest) 下载 `aemeath-claude.exe`（v1.0.4），放到目标目录：

```
.claude/aemeath/
├── aemeath-claude.exe
├── hooks.json        (从 release 下载)
└── mcp.json          (从 release 下载)
```

### 2. 修复 WebView2Loader.dll

**已知问题**：预编译的 `aemeath-claude.exe` 缺少 `WebView2Loader.dll`，直接运行会报错：

```
aemeath-claude.exe - 系统错误
由于找不到 WebView2Loader.dll，无法继续执行代码。重新安装程序可能会解决此问题。
```

**解决方法**：从系统中复制一份 `WebView2Loader.dll` 到 exe 同目录。

**查找 DLL**（按优先级）：

```bash
# 方法 1：搜索系统常见路径
find "/c/Program Files" -name "WebView2Loader.dll" -maxdepth 5 2>/dev/null

# 方法 2：搜索用户目录
find "/c/Users/$USER" -name "WebView2Loader.dll" -maxdepth 5 2>/dev/null

# 方法 3：搜索 Windows 目录
find "/c/Windows" -name "WebView2Loader.dll" -maxdepth 5 2>/dev/null
```

**常见位置**：
- `C:\Program Files\Common Files\Adobe\Microsoft\EdgeWebView\WebView2Loader.dll`
- `C:\Program Files\Adobe\Adobe Premiere Pro 2026\WebView2Loader.dll`
- `C:\Program Files\Lenovo\AIAgent\lsf\WebView2Loader.dll`

**复制**：

```bash
cp "/c/Program Files/Common Files/Adobe/Microsoft/EdgeWebView/WebView2Loader.dll" \
   ".claude/aemeath/"
```

**架构匹配**：exe 和 DLL 都必须是 x64（PE32+）。验证：

```bash
file .claude/aemeath/aemeath-claude.exe    # 应显示 "PE32+ executable ... x86-64"
file .claude/aemeath/WebView2Loader.dll    # 应显示 "PE32+ executable ... x86-64"
```

### 3. 配置 Claude Code Hooks

将以下 JSON 合并到你的 `~/.claude/settings.json` 的 `hooks` 字段中。

**⚠️ 重要**：请将路径中的 `你的路径` 替换为实际路径。

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "powershell -Command \"if (-not (Get-Process -Name 'aemeath-claude' -ErrorAction SilentlyContinue)) { Start-Process '你的路径/.claude/aemeath/aemeath-claude.exe' -WorkingDirectory '你的路径/.claude/aemeath' }\"",
            "timeout": 10
          },
          {
            "type": "command",
            "command": "powershell -Command \"if (-not (Get-Process -Name 'python' -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -match 'game_guard' })) { Start-Process python -ArgumentList '你的路径/.claude/aemeath/game_guard.py' -WindowStyle Hidden }\"",
            "timeout": 10
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:9527/api/hook/thinking"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:9527/api/hook/working"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:9527/api/hook/done"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:9527/api/hook/idle"
          }
        ]
      }
    ],
    "PermissionRequest": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:9527/api/hook/permission"
          }
        ]
      }
    ]
  }
}
```

> **注意**：如果你的 `settings.json` 已有 `hooks` 字段，需要将上述内容合并进去，不要覆盖已有的 hook（如 frontmatter 检查、wikilink 检查等）。

### 4. 配置 MCP

将以下内容添加到项目根目录的 `.mcp.json` 的 `mcpServers` 字段中：

```json
{
  "aemeath": {
    "type": "http",
    "url": "http://127.0.0.1:9528/mcp"
  }
}
```

### 完成

重启 Claude Code，爱弥斯应该会自动出现在你的桌面上！

---

## 功能详解

### 15 种动画状态

| 状态 | 触发条件 | 气泡文字 | HTTP 端点 |
|------|----------|----------|-----------|
| `idle` | Claude 空闲 | — | `/api/hook/idle` |
| `chatting` | 用户发消息 | "正在组织回复..." | `/api/hook/thinking` |
| `running` | 工具执行（默认） | "正在执行命令..." | `/api/hook/working` |
| `fetching` | WebFetch | "正在获取网络内容..." | `/api/hook/working` |
| `searching` | WebSearch | "正在搜索网络..." | `/api/hook/working` |
| `building` | Write / Edit | "正在构建..." | `/api/hook/working` |
| `analyzing` | Agent / Task | "正在分析..." | `/api/hook/working` |
| `celebrating` | 工具完成 | "太棒了!" | `/api/hook/done` |
| `waving` | 权限请求 | "等待指示..." | `/api/hook/permission` |
| `jumping` | 空闲随机动画 | — | 自动 |
| `failed` | 错误状态 | "好像出问题了..." | Rust 状态机 |
| `waiting` | 等待状态 | — | Rust 状态机 |
| `review` | 审查状态 | — | Rust 状态机 |
| `running-right` | 向右移动 | — | Rust 状态机 |
| `running-left` | 向左移动 | — | Rust 状态机 |

### 持久化气泡

**原版问题**：气泡显示 4 秒后自动消失，用户容易错过重要信息。

**改造**：在 `bubble.js` 中新增 `showPersistent(text)` 方法，不设置 auto-hide timer。

### 权限渐进恢复

**原版问题**：权限被拒绝/打断后 Claude Code 不发送事件，爱弥斯卡在 "等待指示..." 最多 120 秒。

**改造**：15 秒后气泡变淡（opacity → 0.4），60 秒后完全清除。

### Idle 提醒

空闲 5 分钟后，每 2-3 分钟随机显示提醒文案（8 条可选）。

### 双击打开 Obsidian

双击爱弥斯通过 `obsidian://` 协议打开 Obsidian。

**Rust 后端** (`main.rs`)：

```rust
#[tauri::command]
fn open_obsidian() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", "obsidian://open?vault=Obsidian%20Vault"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
```

**前端** (`app.js`) — 50ms 延迟区分双击与拖拽：

```javascript
let lastClickTime = 0;
let clickTimer = null;
document.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    const now = Date.now();
    if (now - lastClickTime < 200) {
        clearTimeout(clickTimer);
        clickTimer = null;
        lastClickTime = 0;
        ipc.invoke('open_obsidian');
    } else {
        lastClickTime = now;
        clickTimer = setTimeout(() => {
            ipc.invoke('start_drag');
        }, 50);
    }
});
```

**踩坑记录 — Obsidian 激活方案**：

| 尝试 | 方案 | 结果 |
|------|------|------|
| 1 | `cmd /C start obsidian://` | URL 被默认浏览器打开 |
| 2 | `Obsidian.exe vault_path` | 已运行时弹仓库选择器 |
| 3 | PowerShell + Win32 API | 弹终端窗口，不美观 |
| 4 | `cmd /C start "" "obsidian://open?vault=..."` + `CREATE_NO_WINDOW` | ✅ 成功 |

**踩坑记录 — 双击 vs 拖拽共存**：

| 尝试 | 方案 | 拖拽延迟 | 双击窗口 | 结果 |
|------|------|----------|----------|------|
| 1 | 原生 dblclick 事件 | 0ms | - | `start_dragging()` 捕获鼠标，dblclick 不触发 |
| 2 | mouseup 触发拖拽 | 0ms | - | `start_dragging()` 必须在 mousedown 调用，失效 |
| 3 | 250ms 延迟 | 250ms | 300ms | 能用但延迟明显 |
| 4 | 右键触发 | 0ms | - | 能用但占用右键 |
| 5 | 100ms 延迟 | 100ms | 300ms | 能用但拖拽脱节 |
| 6 | 50ms 延迟 | 50ms | 200ms | ✅ 成功，拖拽无感，双击可触发 |

> **关键发现**：Tauri v2 的 `start_dragging()` 是阻塞调用，捕获鼠标后不会释放给 webview，导致 dblclick 事件丢失。Windows `start` 命令的 `""` 是窗口标题参数，省略会导致 URL 被误解。`CREATE_NO_WINDOW` (0x08000000) 可隐藏 cmd 进程窗口。50ms ≈ 3 帧（60fps），人类几乎无法感知；200ms 双击窗口需要快速连点但可实现。

### 游戏自动隐身

`game_guard.py` 每 3 秒用 Win32 API 检测前台窗口是否全屏，自动隐藏/显示。已加入 SessionStart hook 自动启动。

---

## HTTP API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/current` | GET | 获取当前动画和气泡状态 |
| `/api/state` | POST | 设置宠物状态 |
| `/api/heartbeat` | GET | 心跳检测 |
| `/api/hook/thinking` | POST | 用户发消息 → 聊天动画 |
| `/api/hook/working` | POST | 工具执行中 → 跑步/构建/搜索动画 |
| `/api/hook/done` | POST | 工具完成 → 庆祝动画 |
| `/api/hook/idle` | POST | 回复结束 → 待机动画 |
| `/api/hook/permission` | POST | 权限请求 → 挥手动画 |
| `/api/hide` | POST | 隐藏窗口 |
| `/api/show` | POST | 显示窗口 |

**MCP 端口**：`127.0.0.1:9528`

---

## 精灵图合并

合并两个精灵图源：Q 版 9 状态（行 0-8）+ 原版 6 独有状态（行 9-14）= 15 种状态。

精灵图规格：1536×3120，8 列 × 15 行，每格 192×208，WEBP 格式。

```bash
pip install Pillow
python merge_sprites.py      # 合并精灵图
python generate_previews.py  # 生成预览 GIF
```

---

## 构建

```bash
# 安装 Rust（首次）
winget install Rustlang.Rustup
# 或
curl -o /tmp/rustup-init.exe https://win.rustup.rs/x86_64
/tmp/rustup-init.exe -y --default-toolchain stable

# 刷新 PATH
export PATH="$HOME/.cargo/bin:$PATH"

# 安装 Tauri CLI（首次约 15 分钟）
cargo install tauri-cli --version "^2"

# 构建
cd .claude/aemeath/source
npm install
cargo build --manifest-path src-tauri/Cargo.toml --release
```

产出：`src-tauri/target/release/aemeath-claude.exe`

---

## Token 消耗分析

| Hook 类型 | 数量 | 上下文污染 | Token 消耗 |
|-----------|------|-----------|-----------|
| `type: "http"` | 6 个 | ✅ 零 | ✅ 零 |
| `type: "command"` | 2 个 | ⚠️ 偶发 | ⚠️ 最多 ~160 tokens/回合 |

爱弥斯的 6 个核心 hook 全部是 HTTP 类型，完全不经过 Claude 对话上下文。

---

## 排错指南

### "找不到 WebView2Loader.dll"

搜索系统：`find "/c/Program Files" -name "WebView2Loader.dll" -maxdepth 5`，复制到 exe 同目录。确认架构匹配（x64）。

### SmartScreen 拦截

点击"更多信息" → "仍要运行"。

### 端口 9527 未监听

检查进程：`Get-Process -Name 'aemeath*'`。检查端口：`netstat -an | Select-String ':9527'`。重启 exe。

### Windows 终端编码乱码

命令前加 `PYTHONIOENCODING=utf-8`。

### Hook 脚本找不到文件

使用 `$CLAUDE_PROJECT_DIR` 环境变量：`cd "$CLAUDE_PROJECT_DIR" && python ".claude/scripts/..."`

---

## 文件清单

```
.claude/aemeath/
├── aemeath-claude.exe          ← 当前运行版本
├── aemeath-claude.exe.bak      ← 原版备份
├── WebView2Loader.dll          ← WebView2 加载器
├── game_guard.py               ← 游戏自动隐身脚本
├── merge_sprites.py            ← 精灵图合并脚本
├── generate_previews.py        ← 预览 GIF 生成脚本
├── preview/                    ← 预览 GIF 和总览图
└── source/                     ← 修改后的源码
    ├── src/                    ← 前端（HTML/CSS/JS）
    └── src-tauri/src/          ← 后端（Rust）
```

---

## 致谢

- [77wilNd/aemeath_withclaude](https://github.com/77wilNd/aemeath_withclaude) — 原版桌宠，MIT 许可
- [cuNuo/aemeath-mini-codex-pet](https://github.com/cuNuo/aemeath-mini-codex-pet) — Q 版精灵图素材，MIT 许可
- [lzy-buaa-jdi/aemeath](https://github.com/lzy-buaa-jdi/aemeath) — 像素小人素材来源，MIT License

## 来源与授权

- 像素小人素材来源：[lzy-buaa-jdi/aemeath](https://github.com/lzy-buaa-jdi/aemeath)，MIT License
- 爱弥斯、《鸣潮》及相关官方视觉设定归其权利方所有
- 本仓库仅包含整理后的桌宠代码、精灵图集，不含官方立绘原图

## License

MIT
