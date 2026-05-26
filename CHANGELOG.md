# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2026-05-26

### Added
- 新精灵图适配 + 闲置动画轮换（jumping/waving 随机切换）
- game_guard 睡眠恢复检测（两次轮询间隔 >8s 判定为睡眠恢复，自动检查进程存活）
- bubble.js fadeTimer 管理，修复 persistent→非 persistent 切换时的队列残留
- troubleshooting.md 游戏隐身排错文档

### Fixed
- 空闲随机动画优化：移除 chatting（无对应精灵帧），播放时长 2s→3s，触发间隔 15~45s→20~60s
- Chatting 状态动画映射改为 waiting（借用 6 帧动画）
- game_guard 隐藏期间阻止 Claude hook 覆盖可见状态（新增 force_hidden 标志）

## [0.1.0] - 2026-05-18

### Added
- 首次发布
- Q 版像素爱弥斯桌宠，与 Claude Code 实时联动
- HTTP Server (:9527) + MCP Server (:9528)
- 状态机：idle / working / error / chatting
- 系统托盘 + 拖拽移动
- 气泡消息队列
