export type EfCommandType =
  | 'add-migration'
  | 'update-database'
  | 'remove-migration'
  | 'generate-sql-script'
  | 'drop-database'

export type EfCommandRequest = {
  profileId: string
  commandType: EfCommandType
  projectPath: string
  startupProjectPath: string
  context?: string
  migrationName?: string
  targetMigration?: string
  fromMigration?: string
  toMigration?: string
  output?: string
  outputDir?: string
  namespace?: string
  connection?: string
  framework?: string
  configuration?: string
  runtime?: string
  noBuild?: boolean
  verbose?: boolean
  idempotent?: boolean
  noTransactions?: boolean
  force?: boolean
  dryRun?: boolean
  additionalArgs?: string[]
  forwardedArgs?: string[]
}

export type CommandPreview = {
  command: string
  args: string[]
}

export type CommandExecutionResult = {
  command: string
  success: boolean
  exitCode: number
  stdout: string
  stderr: string
  durationMs: number
}

export type CommandOutputChunk = {
  stream: 'stdout' | 'stderr'
  chunk: string
}

export type EnvironmentStatus = {
  dotnet: {
    available: boolean
    output: string
  }
  dotnetEf: {
    available: boolean
    output: string
  }
}

export type HistoryRecord = {
  id: string
  profileId: string
  commandType: EfCommandType
  command: string
  success: boolean
  exitCode: number
  durationMs: number
  createdAt: string
  stdout: string
  stderr: string
}
