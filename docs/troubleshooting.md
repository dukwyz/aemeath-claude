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
