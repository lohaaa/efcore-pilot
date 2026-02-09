export type ProjectProfile = {
  id: string
  name: string
  group?: string
  tags: string[]
  workspacePath: string
  projectPath: string
  migrationsProjectPath: string
  startupProjectPath: string
  migrationsDirectory?: string
  dbContext: string
  createdAt: string
  updatedAt: string
  lastUsedAt?: string
  enabled: boolean
}

export type ProfileFormModel = {
  id?: string
  name: string
  group?: string
  tagsText: string
  workspacePath: string
  projectPath: string
  migrationsProjectPath: string
  startupProjectPath: string
  migrationsDirectory?: string
  dbContext: string
  enabled: boolean
}
