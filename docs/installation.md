# 完整安装指南

## 1. 下载

从 [Releases](https://github.com/dukwyz/aemeath-claude/releases/latest) 下载 `aemeath-claude.exe`，放到目标目录：

```
.claude/aemeath/
├── aemeath-claude.exe
├── hooks.json        (从 release 下载)
└── mcp.json          (从 release 下载)
```

## 2. 修复 WebView2Loader.dll

**已知问题**：预编译的 `aemeath-claude.exe` 缺少 `WebView2Loader.dll`，直接运行会报错：

```
aemeath-claude.exe - 系统错误
由于找不到 WebView2Loader.dll，无法继续执行代码。
```

**解决方法**：从系统中复制一份 `WebView2Loader.dll` 到 exe 同目录。

**常见位置**：
- `C:\Program Files\Common Files\Adobe\Microsoft\EdgeWebView\WebView2Loader.dll`
- `C:\Program Files\Adobe\Adobe Premiere Pro 2026\WebView2Loader.dll`

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

## 3. 配置 Claude Code Hooks

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

> **注意**：如果你的 `settings.json` 已有 `hooks` 字段，需要将上述内容合并进去，不要覆盖已有的 hook。

## 4. 配置 MCP

将以下内容添加到项目根目录的 `.mcp.json` 的 `mcpServers` 字段中：

```json
{
  "aemeath": {
    "type": "http",
    "url": "http://127.0.0.1:9528/mcp"
  }
}
```

## 5. 完成

重启 Claude Code，爱弥斯应该会自动出现在你的桌面上！
