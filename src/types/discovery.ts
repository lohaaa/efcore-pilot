export type ScanWorkspaceRequest = {
  path: string
}

export type WorkspaceProjectInfo = {
  name: string
  path: string
  relativePath: string
  directory: string
  targetFrameworks: string[]
  packageReferences: string[]
  isStartupCandidate: boolean
  isMigrationsCandidate: boolean
  dbContexts: string[]
  migrationDirectories: string[]
  migrationNames: string[]
}

export type WorkspaceScanResult = {
  workspaceRoot: string
  solutionPath?: string
  projects: WorkspaceProjectInfo[]
}
