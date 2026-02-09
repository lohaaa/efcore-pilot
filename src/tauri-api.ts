import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  CommandExecutionResult,
  CommandOutputChunk,
  CommandPreview,
  EfCommandRequest,
  EnvironmentStatus
} from './types/command'
import type { ScanWorkspaceRequest, WorkspaceScanResult } from './types/discovery'

function isTauriRuntime() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

const EF_COMMAND_OUTPUT_EVENT = 'ef-command-output'

export async function detectEnvironment(): Promise<EnvironmentStatus> {
  if (!isTauriRuntime()) {
    return {
      dotnet: { available: false, output: 'Not running inside Tauri runtime.' },
      dotnetEf: { available: false, output: 'Not running inside Tauri runtime.' }
    }
  }
  return invoke<EnvironmentStatus>('detect_environment')
}

export async function previewEfCommand(request: EfCommandRequest): Promise<CommandPreview> {
  if (!isTauriRuntime()) {
    const args = ['ef', request.commandType]
    return {
      command: `dotnet ${args.join(' ')}`,
      args
    }
  }
  return invoke<CommandPreview>('preview_ef_command', { request })
}

export async function listenEfCommandOutput(
  handler: (payload: CommandOutputChunk) => void
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) {
    return () => {}
  }

  return listen<CommandOutputChunk>(EF_COMMAND_OUTPUT_EVENT, (event) => {
    handler(event.payload)
  })
}

export async function interruptEfCommand(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false
  }
  return invoke<boolean>('interrupt_ef_command')
}

export async function executeEfCommand(request: EfCommandRequest): Promise<CommandExecutionResult> {
  if (!isTauriRuntime()) {
    return {
      command: 'dotnet (mock mode)',
      success: false,
      exitCode: -1,
      durationMs: 0,
      stdout: '',
      stderr: 'Execution is only available inside Tauri runtime.'
    }
  }
  return invoke<CommandExecutionResult>('execute_ef_command', { request })
}

export async function scanWorkspace(request: ScanWorkspaceRequest): Promise<WorkspaceScanResult> {
  if (!isTauriRuntime()) {
    return {
      workspaceRoot: request.path,
      solutionPath: undefined,
      projects: []
    }
  }
  return invoke<WorkspaceScanResult>('scan_workspace', { request })
}
