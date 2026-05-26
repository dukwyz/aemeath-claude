# 排错指南

## "找不到 WebView2Loader.dll"

搜索系统：`find "/c/Program Files" -name "WebView2Loader.dll" -maxdepth 5`，复制到 exe 同目录。确认架构匹配（x64）。

## SmartScreen 拦截

点击"更多信息" → "仍要运行"。

## 端口 9527 未监听

检查进程：`Get-Process -Name 'aemeath*'`。检查端口：`netstat -an | Select-String ':9527'`。重启 exe。

## Windows 终端编码乱码

命令前加 `PYTHONIOENCODING=utf-8`。

## Hook 脚本找不到文件

使用 `$CLAUDE_PROJECT_DIR` 环境变量：`cd "$CLAUDE_PROJECT_DIR" && python ".claude/scripts/..."`

## 游戏隐身不生效

检查 `game_guard.py` 是否在运行：`Get-Process -Name 'python*'`。

如果不在运行，查看日志：`cat .claude/aemeath/game_guard.log`。

**已知问题**：`Start-Process python` 在某些环境下会导致进程立即退出。解决方案：使用 `pythonw.exe` + `&` 调用：

```powershell
& 'C:\Python314\pythonw.exe' 'D:/path/to/.claude/aemeath/game_guard.py'
```
