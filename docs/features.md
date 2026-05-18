# 功能详解

## 15 种动画状态

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

## 持久化气泡

**原版问题**：气泡显示 4 秒后自动消失，用户容易错过重要信息。

**改造**：在 `bubble.js` 中新增 `showPersistent(text)` 方法，不设置 auto-hide timer。工具执行期间气泡持续显示。

## 权限渐进恢复

**原版问题**：权限被拒绝/打断后 Claude Code 不发送事件，爱弥斯卡在 "等待指示..." 最多 120 秒。

**改造**：15 秒后气泡变淡（opacity → 0.4），60 秒后完全清除。

## Idle 提醒

空闲 5 分钟后，每 2-3 分钟随机显示提醒文案（8 条可选）。

## 双击打开 Obsidian

双击爱弥斯通过 `obsidian://` 协议打开 Obsidian。50ms 延迟区分双击与拖拽。

### 踩坑记录

**Obsidian 激活方案**：

| 尝试 | 方案 | 结果 |
|------|------|------|
| 1 | `cmd /C start obsidian://` | URL 被默认浏览器打开 |
| 2 | `Obsidian.exe vault_path` | 已运行时弹仓库选择器 |
| 3 | PowerShell + Win32 API | 弹终端窗口，不美观 |
| 4 | `cmd /C start "" "obsidian://open?vault=..."` + `CREATE_NO_WINDOW` | ✅ 成功 |

**双击 vs 拖拽共存**：

| 尝试 | 方案 | 拖拽延迟 | 双击窗口 | 结果 |
|------|------|----------|----------|------|
| 1 | 原生 dblclick 事件 | 0ms | - | `start_dragging()` 捕获鼠标，dblclick 不触发 |
| 2 | mouseup 触发拖拽 | 0ms | - | `start_dragging()` 必须在 mousedown 调用，失效 |
| 3 | 250ms 延迟 | 250ms | 300ms | 能用但延迟明显 |
| 4 | 右键触发 | 0ms | - | 能用但占用右键 |
| 5 | 100ms 延迟 | 100ms | 300ms | 能用但拖拽脱节 |
| 6 | 50ms 延迟 | 50ms | 200ms | ✅ 成功，拖拽无感，双击可触发 |

> **关键发现**：Tauri v2 的 `start_dragging()` 是阻塞调用，捕获鼠标后不会释放给 webview，导致 dblclick 事件丢失。

## 游戏自动隐身

`game_guard.py` 每 3 秒用 Win32 API 检测前台窗口是否全屏，自动隐藏/显示。已加入 SessionStart hook 自动启动。

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

## Token 消耗分析

| Hook 类型 | 数量 | 上下文污染 | Token 消耗 |
|-----------|------|-----------|-----------|
| `type: "http"` | 6 个 | ✅ 零 | ✅ 零 |
| `type: "command"` | 2 个 | ⚠️ 偶发 | ⚠️ 最多 ~160 tokens/回合 |

爱弥斯的 6 个核心 hook 全部是 HTTP 类型，完全不经过 Claude 对话上下文。
