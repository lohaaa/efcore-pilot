import { open, save } from '@tauri-apps/plugin-dialog'

type DialogSelection = string | string[] | null

function normalizeDialogSelection(selection: DialogSelection): string | null {
  if (!selection) {
    return null
  }
  if (Array.isArray(selection)) {
    return selection[0] ?? null
  }
  return selection
}

function normalizeDefaultPath(path?: string): string | undefined {
  const trimmed = path?.trim()
  return trimmed ? trimmed : undefined
}

export async function pickDirectory(defaultPath?: string): Promise<string | null> {
  const selection = await open({
    multiple: false,
    directory: true,
    defaultPath: normalizeDefaultPath(defaultPath)
  })
  return normalizeDialogSelection(selection)
}

export async function pickSolutionFile(defaultPath?: string): Promise<string | null> {
  const selection = await open({
    multiple: false,
    directory: false,
    defaultPath: normalizeDefaultPath(defaultPath),
    filters: [{ name: 'Solution', extensions: ['sln'] }]
  })
  return normalizeDialogSelection(selection)
}

export async function pickProjectFile(defaultPath?: string): Promise<string | null> {
  const selection = await open({
    multiple: false,
    directory: false,
    defaultPath: normalizeDefaultPath(defaultPath),
    filters: [{ name: 'C# Project', extensions: ['csproj'] }]
  })
  return normalizeDialogSelection(selection)
}

export async function pickOutputFile(defaultPath?: string): Promise<string | null> {
  const selection = await save({
    defaultPath: normalizeDefaultPath(defaultPath),
    filters: [{ name: 'SQL Script', extensions: ['sql'] }]
  })
  return selection ?? null
}
