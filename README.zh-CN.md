# EfCorePilot

[English](./README.md) | [简体中文](./README.zh-CN.md)

用于统一管理多个项目 EF Core 迁移流程的跨平台桌面工具。

## 技术栈

- Tauri 2
- Vue 3 + TypeScript
- Naive UI
- Pinia + Vue I18n

## EF Core 版本兼容与建议

- 命令兼容范围：`.NET 8+` 下的 EF Core CLI 迁移工作流（含 EF Core 8/9/10）。
- 生产环境建议：优先选择 LTS 版本（当前可选 `.NET 8` 或 `.NET 10`）。
- 版本匹配原则：`dotnet-ef` 与 `Microsoft.EntityFrameworkCore.*` 保持同主版本。
- 多目标框架项目：执行命令时需明确目标框架（本工具会在检测到多目标时提供选择）。

## 当前开发范围（Phase 1）

- 项目配置管理
- EF Core 命令构建（预览 + 执行）
- 环境检查（`dotnet`、`dotnet-ef`）
- 执行历史（本地持久化）
- 英文 + 简体中文切换

## 计划功能（低优先级）

- Scaffold DbContext（逆向工程）

## 运行

```bash
pnpm install
pnpm tauri dev
```

## 说明

- 命令执行功能在 Tauri 运行时可用。
- 浏览器模式（`pnpm dev`）提供 UI 与模拟命令执行。
- 当前数据存储在浏览器本地存储中。
- 受限于设备，macOS 未经测试。
