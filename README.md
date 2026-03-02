# EfCorePilot

[English](./README.md) | [简体中文](./README.zh-CN.md)

Cross-platform desktop tool for managing EF Core migration workflows across multiple projects.

## Tech Stack

- Tauri 2
- Vue 3 + TypeScript
- Naive UI
- Pinia + Vue I18n

## EF Core Compatibility

- Command support: EF Core CLI migration workflows on `.NET 8+` (including EF Core 8/9/10).
- Production recommendation: prefer LTS releases (currently `.NET 8` or `.NET 10`).
- Version alignment: keep `dotnet-ef` on the same major version as `Microsoft.EntityFrameworkCore.*`.
- Multi-target projects: explicitly select a target framework when running commands (this app shows a selector when needed).

## Current Development Scope (Phase 1)

- Project profiles management
- EF Core command builder (preview + execute)
- Environment checks (`dotnet`, `dotnet-ef`)
- Execution history (local persistence)
- English + Simplified Chinese switch

## Planned Features (Low Priority)

- Scaffold DbContext (Reverse Engineering)

## Run

```bash
pnpm install
pnpm tauri dev
```

## Notes

- Command execution is available in Tauri runtime.
- Browser-only mode (`pnpm dev`) provides UI with mock command execution.
- Data is stored locally in browser storage for now.
- Due to hardware limitations, macOS is untested.
