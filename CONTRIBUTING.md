# 贡献指南

欢迎提交 Issue 和 PR！

## 开发环境

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) >= 18
- [Tauri CLI](https://tauri.app/) v2

## 本地开发

```bash
git clone https://github.com/dukwyz/aemeath-claude.git
cd aemeath-claude
npm install
cargo build --manifest-path src-tauri/Cargo.toml --release
```

## 提交规范

- 功能新增：`feat: xxx`
- Bug 修复：`fix: xxx`
- 文档更新：`docs: xxx`
