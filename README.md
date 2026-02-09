# EfCorePilot

Cross-platform desktop tool for managing EF Core migration workflows across multiple projects.

## Tech Stack

- Tauri 2
- Vue 3 + TypeScript
- Naive UI
- Pinia + Vue I18n

## Current Development Scope (Phase 1)

- Project profiles management
- EF Core command builder (preview + execute)
- Environment checks (`dotnet`, `dotnet-ef`)
- Execution history (local persistence)
- English + Simplified Chinese switch

## Run

```bash
pnpm install
pnpm tauri dev
```

## Notes

- Command execution is available in Tauri runtime.
- Browser-only mode (`pnpm dev`) provides UI with mock command execution.
- Data is stored locally in browser storage for now.
