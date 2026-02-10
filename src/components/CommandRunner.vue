<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMessage } from 'naive-ui'
import { confirm } from '@tauri-apps/plugin-dialog'
import { useProfilesStore } from '../stores/profiles'
import { useHistoryStore } from '../stores/history'
import { executeEfCommand, interruptEfCommand, listenEfCommandOutput, previewEfCommand, scanWorkspace } from '../tauri-api'
import type {
  CommandExecutionResult,
  CommandPreview,
  EfCommandRequest,
  EfCommandType
} from '../types/command'
import type { WorkspaceProjectInfo, WorkspaceScanResult } from '../types/discovery'
import { pickDirectory, pickOutputFile } from '../utils/dialog'

const { t } = useI18n()
const message = useMessage()
const profilesStore = useProfilesStore()
const historyStore = useHistoryStore()

const SQL_SCRIPT_OUTPUT_CACHE_KEY = 'efp_sql_script_output_cache_v1'

type SqlScriptOutputCache = Record<string, string>

function readSqlScriptOutputCache(): SqlScriptOutputCache {
  try {
    const raw = localStorage.getItem(SQL_SCRIPT_OUTPUT_CACHE_KEY)
    if (!raw) {
      return {}
    }

    const parsed = JSON.parse(raw) as SqlScriptOutputCache
    if (!parsed || typeof parsed !== 'object') {
      return {}
    }

    return parsed
  } catch {
    return {}
  }
}

function writeSqlScriptOutputCache(cache: SqlScriptOutputCache) {
  localStorage.setItem(SQL_SCRIPT_OUTPUT_CACHE_KEY, JSON.stringify(cache))
}

const commandType = ref<EfCommandType>('add-migration')
const running = ref(false)
const interrupting = ref(false)
const interruptRequested = ref(false)
const preview = ref<CommandPreview | null>(null)
const result = ref<CommandExecutionResult | null>(null)
const liveExecutionOutput = ref('')
const executionOutputContainerRef = ref<HTMLElement | null>(null)
let pendingExecutionOutput = ''
let executionOutputFlushTimer: number | null = null
let unlistenCommandOutput: (() => void) | null = null

const loadingMetadata = ref(false)
const workspaceScanResult = ref<WorkspaceScanResult | null>(null)
const useConnectionOverride = ref(false)
const advancedExpandedNames = ref<string[]>([])

const form = reactive({
  migrationName: '',
  targetMigration: '',
  fromMigration: '',
  toMigration: '',
  output: '',
  outputDir: '',
  connection: '',
  context: '',
  framework: '',
  configuration: '',
  noBuild: false,
  verbose: false,
  idempotent: false,
  noTransactions: false,
  force: false,
  dryRun: false
})

const selectedProfile = computed(() => profilesStore.selectedProfile)

const commandOptions = computed(() => [
  { label: t('command.addMigration'), value: 'add-migration' },
  { label: t('command.updateDatabase'), value: 'update-database' },
  { label: t('command.removeMigration'), value: 'remove-migration' },
  { label: t('command.generateSqlScript'), value: 'generate-sql-script' },
  { label: t('command.dropDatabase'), value: 'drop-database' }
])

const configurationOptions = computed(() => [
  { label: t('command.configurationDefault'), value: '' },
  { label: 'Debug', value: 'Debug' },
  { label: 'Release', value: 'Release' }
])

const executionOutputText = computed(() => {
  if (liveExecutionOutput.value) {
    return liveExecutionOutput.value
  }

  const stdout = result.value?.stdout || ''
  const stderr = result.value?.stderr || ''
  if (stdout && stderr) {
    return `${stdout}\n${stderr}`
  }
  return stdout || stderr
})

function scrollExecutionOutputToLatest() {
  void nextTick(() => {
    const textarea = executionOutputContainerRef.value?.querySelector('textarea')
    if (!textarea) {
      return
    }

    textarea.scrollTop = textarea.scrollHeight
  })
}

function flushExecutionOutputBuffer() {
  if (!pendingExecutionOutput) {
    return
  }

  liveExecutionOutput.value += pendingExecutionOutput
  pendingExecutionOutput = ''
}

function scheduleExecutionOutputFlush() {
  if (executionOutputFlushTimer !== null) {
    return
  }

  executionOutputFlushTimer = window.setTimeout(() => {
    executionOutputFlushTimer = null
    flushExecutionOutputBuffer()
  }, 60)
}

function appendExecutionOutputChunk(chunk: string) {
  pendingExecutionOutput += chunk

  if (pendingExecutionOutput.length >= 8192) {
    if (executionOutputFlushTimer !== null) {
      window.clearTimeout(executionOutputFlushTimer)
      executionOutputFlushTimer = null
    }
    flushExecutionOutputBuffer()
    return
  }

  scheduleExecutionOutputFlush()
}

function resetExecutionOutputBuffer() {
  pendingExecutionOutput = ''
  if (executionOutputFlushTimer !== null) {
    window.clearTimeout(executionOutputFlushTimer)
    executionOutputFlushTimer = null
  }
}

function normalizePath(value: string) {
  return value.trim().replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
}

function normalizeFilePath(value: string) {
  return value.trim().replace(/\\/g, '/').replace(/\/+$/, '')
}

function isAbsolutePath(value: string) {
  const normalized = normalizeFilePath(value)
  return normalized.startsWith('/') || /^[a-zA-Z]:\//.test(normalized) || normalized.startsWith('//')
}

function joinWithProjectRoot(projectRoot: string, value: string) {
  const base = normalizeFilePath(projectRoot)
  const normalizedValue = normalizeFilePath(value).replace(/^[/\\]+/, '')

  if (!base) {
    return normalizedValue
  }

  if (!normalizedValue) {
    return base
  }

  return `${base}/${normalizedValue}`
}

function getProjectRootPath(projectPath: string) {
  const normalized = normalizeFilePath(projectPath)
  if (!normalized) {
    return ''
  }

  const lower = normalized.toLowerCase()
  if (!lower.endsWith('.csproj') && !lower.endsWith('.sln')) {
    return normalized
  }

  const slashIndex = normalized.lastIndexOf('/')
  if (slashIndex < 0) {
    return normalized
  }

  return normalized.slice(0, slashIndex)
}

function getCurrentProjectRoot() {
  const scannedProjectDirectory = selectedProjectInfo.value?.directory?.trim()
  if (scannedProjectDirectory) {
    return normalizeFilePath(scannedProjectDirectory)
  }

  const profile = selectedProfile.value
  if (!profile) {
    return ''
  }

  const projectPath = (profile.migrationsProjectPath || profile.projectPath).trim()
  if (projectPath) {
    const projectRoot = getProjectRootPath(projectPath)
    if (projectRoot) {
      return projectRoot
    }
  }

  const workspacePath = profile.workspacePath.trim()
  if (workspacePath) {
    return getProjectRootPath(workspacePath)
  }

  return ''
}

function getWorkspaceRootPath() {
  const scannedWorkspaceRoot = workspaceScanResult.value?.workspaceRoot?.trim()
  if (scannedWorkspaceRoot) {
    return normalizeFilePath(scannedWorkspaceRoot)
  }

  const profile = selectedProfile.value
  if (!profile) {
    return ''
  }

  const workspacePath = profile.workspacePath.trim()
  if (!workspacePath) {
    return ''
  }

  return getProjectRootPath(workspacePath)
}

function resolveDefaultSqlScriptOutputPath() {
  const workspaceRoot = getWorkspaceRootPath()
  if (!workspaceRoot) {
    return 'script.sql'
  }

  return joinWithProjectRoot(workspaceRoot, 'script.sql')
}

function resolveCachedSqlScriptOutputPath(profileId: string) {
  const cache = readSqlScriptOutputCache()
  const cached = cache[profileId]?.trim()
  return cached || ''
}

function resolveProfileSqlScriptOutputPath(profileId: string) {
  return resolveCachedSqlScriptOutputPath(profileId) || resolveDefaultSqlScriptOutputPath()
}

function persistProfileSqlScriptOutputPath(profileId: string, outputPath: string) {
  const normalized = outputPath.trim()
  if (!normalized) {
    return
  }

  const cache = readSqlScriptOutputCache()
  cache[profileId] = normalized
  writeSqlScriptOutputCache(cache)
}

function syncSqlScriptOutputPathForProfile(profileId: string) {
  form.output = resolveProfileSqlScriptOutputPath(profileId)
}

function resolvePathFromProjectRoot(value: string) {
  const trimmed = value.trim()
  if (!trimmed) {
    return ''
  }

  if (isAbsolutePath(trimmed)) {
    return normalizeFilePath(trimmed)
  }

  const projectRoot = getCurrentProjectRoot()
  if (!projectRoot) {
    return ''
  }

  return joinWithProjectRoot(projectRoot, trimmed)
}

function getParentDirectory(pathValue: string) {
  const normalized = normalizeFilePath(pathValue)
  const slashIndex = normalized.lastIndexOf('/')
  if (slashIndex < 0) {
    return normalized
  }

  return normalized.slice(0, slashIndex)
}

function resolveOutputFileDefaultPath() {
  const resolvedOutputPath = resolvePathFromProjectRoot(form.output)
  if (resolvedOutputPath) {
    return resolvedOutputPath
  }

  const resolvedOutputDirectory = resolvePathFromProjectRoot(form.outputDir)
  if (resolvedOutputDirectory) {
    return resolvedOutputDirectory
  }

  const profile = selectedProfile.value
  if (profile) {
    const resolvedMigrationDirectory = resolvePathFromProjectRoot(profile.migrationsDirectory || '')
    if (resolvedMigrationDirectory) {
      return resolvedMigrationDirectory
    }
  }

  const projectRoot = getCurrentProjectRoot()
  return projectRoot || undefined
}

function resolveOutputDirectoryDefaultPath() {
  const resolvedOutputDirectory = resolvePathFromProjectRoot(form.outputDir)
  if (resolvedOutputDirectory) {
    return resolvedOutputDirectory
  }

  const profile = selectedProfile.value
  if (profile) {
    const resolvedMigrationDirectory = resolvePathFromProjectRoot(profile.migrationsDirectory || '')
    if (resolvedMigrationDirectory) {
      return resolvedMigrationDirectory
    }
  }

  const resolvedOutputPath = resolvePathFromProjectRoot(form.output)
  if (resolvedOutputPath) {
    return getParentDirectory(resolvedOutputPath)
  }

  const projectRoot = getCurrentProjectRoot()
  return projectRoot || undefined
}

function normalizeMigrationValue(value: string) {
  const trimmed = value.trim()
  if (!trimmed) {
    return undefined
  }
  if (trimmed.toLowerCase() === 'latest') {
    return undefined
  }
  return trimmed
}

function normalizeMigrationNameInput(value: string) {
  let normalized = value.trim()
  if (!normalized) {
    return ''
  }

  let previous = ''
  while (normalized && normalized !== previous) {
    previous = normalized
    normalized = normalized
      .replace(/^(预计迁移文件名|expected migration file name)\s*[:：]\s*/i, '')
      .replace(/^\d{14}_/, '')
      .trim()
  }

  return normalized
}

function onMigrationNameBlur() {
  form.migrationName = normalizeMigrationNameInput(form.migrationName)
}

function formatMigrationTimestampPrefix(date: Date) {
  const year = date.getFullYear().toString().padStart(4, '0')
  const month = (date.getMonth() + 1).toString().padStart(2, '0')
  const day = date.getDate().toString().padStart(2, '0')
  const hours = date.getHours().toString().padStart(2, '0')
  const minutes = date.getMinutes().toString().padStart(2, '0')
  const seconds = date.getSeconds().toString().padStart(2, '0')
  return `${year}${month}${day}${hours}${minutes}${seconds}`
}

const migrationFileNamePreview = computed(() => {
  if (commandType.value !== 'add-migration') {
    return ''
  }

  const normalizedName = normalizeMigrationNameInput(form.migrationName)
  if (!normalizedName) {
    return ''
  }

  return `${formatMigrationTimestampPrefix(new Date())}_${normalizedName}`
})

function toOptionList(values: string[]) {
  const unique = Array.from(new Set(values.filter((item) => item.trim())))
  return unique.map((item) => ({ label: item, value: item }))
}

const selectedProjectInfo = computed<WorkspaceProjectInfo | null>(() => {
  const profile = selectedProfile.value
  if (!profile || !workspaceScanResult.value) {
    return null
  }

  const expectedPath = normalizePath(profile.migrationsProjectPath || profile.projectPath)
  if (!expectedPath) {
    return null
  }

  return (
    workspaceScanResult.value.projects.find((project) => normalizePath(project.path) === expectedPath) ?? null
  )
})

const selectedStartupProjectInfo = computed<WorkspaceProjectInfo | null>(() => {
  const profile = selectedProfile.value
  if (!profile || !workspaceScanResult.value) {
    return null
  }

  const expectedPath = normalizePath(profile.startupProjectPath)
  if (!expectedPath) {
    return null
  }

  return (
    workspaceScanResult.value.projects.find((project) => normalizePath(project.path) === expectedPath) ?? null
  )
})

function normalizeFrameworkValues(values: string[]) {
  return Array.from(new Set(values.map((item) => item.trim()).filter(Boolean)))
}

const frameworkCandidates = computed(() => {
  const migrationsFrameworks = normalizeFrameworkValues(selectedProjectInfo.value?.targetFrameworks ?? [])
  const startupFrameworks = normalizeFrameworkValues(selectedStartupProjectInfo.value?.targetFrameworks ?? [])

  if (migrationsFrameworks.length > 0 && startupFrameworks.length > 0) {
    const startupSet = new Set(startupFrameworks)
    const intersection = migrationsFrameworks.filter((item) => startupSet.has(item))
    if (intersection.length > 0) {
      return intersection
    }

    return startupFrameworks
  }

  return startupFrameworks.length > 0 ? startupFrameworks : migrationsFrameworks
})

const shouldShowFrameworkSelector = computed(() => frameworkCandidates.value.length > 1)

const frameworkOptions = computed(() =>
  frameworkCandidates.value.map((item) => ({ label: item, value: item }))
)

const migrationNameOptions = computed(() => toOptionList(selectedProjectInfo.value?.migrationNames ?? []))

const dbContextOptions = computed(() => {
  const profile = selectedProfile.value
  if (!profile) {
    return []
  }

  const contexts = selectedProjectInfo.value?.dbContexts?.length
    ? selectedProjectInfo.value.dbContexts
    : [profile.dbContext]

  return toOptionList(contexts)
})

const targetMigrationOptions = computed(() => {
  return [
    { label: t('command.latestMigration'), value: 'latest' },
    { label: t('command.zeroMigration'), value: '0' },
    ...migrationNameOptions.value.filter((item) => item.value !== '0')
  ]
})

const fromMigrationOptions = computed(() => {
  return [{ label: t('command.zeroMigration'), value: '0' }, ...migrationNameOptions.value]
})

const toMigrationOptions = computed(() => {
  return [
    { label: t('command.latestMigration'), value: 'latest' },
    { label: t('command.zeroMigration'), value: '0' },
    ...migrationNameOptions.value.filter((item) => item.value !== '0')
  ]
})

function alignContextValue() {
  const profile = selectedProfile.value
  if (!profile) {
    form.context = ''
    return
  }

  if (!form.context.trim()) {
    form.context = profile.dbContext
    return
  }

  const values = new Set(dbContextOptions.value.map((item) => item.value))
  if (values.size > 0 && !values.has(form.context)) {
    form.context = values.has(profile.dbContext)
      ? profile.dbContext
      : dbContextOptions.value[0]?.value || profile.dbContext
  }
}

function alignFrameworkValue() {
  const candidates = frameworkCandidates.value

  if (candidates.length <= 1) {
    form.framework = ''
    return
  }

  if (!form.framework.trim() || !candidates.includes(form.framework)) {
    form.framework = candidates[0]
  }
}

async function loadProjectMetadata() {
  const profile = selectedProfile.value
  if (!profile) {
    workspaceScanResult.value = null
    return false
  }

  const scanPath = profile.workspacePath || profile.migrationsProjectPath || profile.projectPath
  if (!scanPath) {
    workspaceScanResult.value = null
    alignContextValue()
    alignFrameworkValue()
    return false
  }

  loadingMetadata.value = true
  try {
    workspaceScanResult.value = await scanWorkspace({ path: scanPath })
    return true
  } catch (error) {
    workspaceScanResult.value = null
    message.warning(`${t('command.loadMetadataFailed')}: ${String(error)}`)
    return false
  } finally {
    loadingMetadata.value = false
    alignContextValue()
    alignFrameworkValue()
  }
}

async function refreshProjectMetadata(manual = false) {
  if (loadingMetadata.value) {
    return
  }

  const success = await loadProjectMetadata()
  if (manual && success) {
    message.success(t('command.metadataRefreshed'))
  }
}

watch(
  selectedProfile,
  (profile) => {
    useConnectionOverride.value = false
    form.connection = ''
    if (!profile) {
      form.context = ''
      form.framework = ''
      form.output = ''
      form.outputDir = ''
      workspaceScanResult.value = null
      return
    }
    form.context = profile.dbContext
    form.framework = ''
    syncSqlScriptOutputPathForProfile(profile.id)
    form.outputDir = profile.migrationsDirectory || ''
    void refreshProjectMetadata()
  },
  { immediate: true }
)

watch(selectedProjectInfo, () => {
  alignContextValue()
})

watch(
  frameworkCandidates,
  () => {
    alignFrameworkValue()
  },
  { immediate: true }
)

watch(commandType, (type) => {
  const profile = selectedProfile.value
  if (!profile) {
    return
  }

  if (type === 'add-migration') {
    if (!form.outputDir.trim()) {
      form.outputDir = profile.migrationsDirectory || ''
    }
    return
  }

  if (type === 'generate-sql-script' && !form.output.trim()) {
    syncSqlScriptOutputPathForProfile(profile.id)
  }
})

watch(executionOutputText, () => {
  scrollExecutionOutputToLatest()
})

onMounted(async () => {
  unlistenCommandOutput = await listenEfCommandOutput((payload) => {
    if (!running.value) {
      return
    }

    if (!payload.chunk) {
      return
    }

    appendExecutionOutputChunk(payload.chunk)
  })
})

onBeforeUnmount(() => {
  flushExecutionOutputBuffer()
  resetExecutionOutputBuffer()

  if (unlistenCommandOutput) {
    unlistenCommandOutput()
    unlistenCommandOutput = null
  }
})

function buildRequest(): EfCommandRequest | null {
  const profile = selectedProfile.value
  if (!profile) {
    return null
  }

  const request: EfCommandRequest = {
    profileId: profile.id,
    commandType: commandType.value,
    projectPath: profile.migrationsProjectPath || profile.projectPath,
    startupProjectPath: profile.startupProjectPath,
    context: form.context.trim() || profile.dbContext,
    migrationName:
      commandType.value === 'add-migration'
        ? normalizeMigrationNameInput(form.migrationName) || undefined
        : undefined,
    targetMigration: normalizeMigrationValue(form.targetMigration),
    fromMigration: normalizeMigrationValue(form.fromMigration),
    toMigration: normalizeMigrationValue(form.toMigration),
    output: form.output.trim() || undefined,
    outputDir: form.outputDir.trim() || profile.migrationsDirectory || undefined,
    connection: useConnectionOverride.value ? form.connection.trim() || undefined : undefined,
    framework: shouldShowFrameworkSelector.value ? form.framework.trim() || undefined : undefined,
    configuration: form.configuration.trim() || undefined,
    noBuild: form.noBuild,
    verbose: form.verbose,
    idempotent: form.idempotent,
    noTransactions: form.noTransactions,
    force: form.force,
    dryRun: form.dryRun
  }

  if (commandType.value === 'add-migration' && !request.migrationName) {
    message.warning(t('command.migrationName'))
    return null
  }

  if (shouldShowFrameworkSelector.value && !request.framework) {
    message.warning(t('command.selectFramework'))
    return null
  }

  return request
}

function onSqlScriptOutputBlur() {
  const profile = selectedProfile.value
  if (!profile || commandType.value !== 'generate-sql-script') {
    return
  }

  if (!form.output.trim()) {
    syncSqlScriptOutputPathForProfile(profile.id)
    return
  }

  persistProfileSqlScriptOutputPath(profile.id, form.output)
}

async function chooseOutputPath() {
  const selected = await pickOutputFile(resolveOutputFileDefaultPath())
  if (!selected) return
  form.output = selected

  const profile = selectedProfile.value
  if (profile && commandType.value === 'generate-sql-script') {
    persistProfileSqlScriptOutputPath(profile.id, selected)
  }
}

async function chooseOutputDirectory() {
  const selected = await pickDirectory(resolveOutputDirectoryDefaultPath())
  if (!selected) return
  form.outputDir = selected
}

async function doPreview() {
  const request = buildRequest()
  if (!request) return

  try {
    preview.value = await previewEfCommand(request)
    message.success(t('common.success'))
  } catch (error) {
    message.error(String(error))
  }
}

async function copyPreviewCommand() {
  const command = preview.value?.command || ''
  if (!command.trim()) {
    return
  }

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(command)
      message.success(t('common.copied'))
      return
    }

    const textarea = document.createElement('textarea')
    textarea.value = command
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.appendChild(textarea)
    textarea.select()
    document.execCommand('copy')
    document.body.removeChild(textarea)
    message.success(t('common.copied'))
  } catch (error) {
    message.error(String(error))
  }
}

async function doInterrupt() {
  if (!running.value || interrupting.value) {
    return
  }

  interrupting.value = true
  try {
    const interrupted = await interruptEfCommand()
    if (interrupted) {
      interruptRequested.value = true
      message.warning(t('command.interruptRequested'))
      return
    }

    message.warning(t('command.interruptNotRunning'))
  } catch (error) {
    message.error(`${t('command.interruptFailed')}: ${String(error)}`)
  } finally {
    interrupting.value = false
  }
}

async function doExecute() {
  const request = buildRequest()
  if (!request) return

  if (request.commandType === 'drop-database') {
    const confirmed = await confirm(t('command.dropDatabaseConfirm'), {
      title: t('command.dropDatabase'),
      kind: 'warning'
    })
    if (!confirmed) {
      return
    }
  }

  const profile = selectedProfile.value
  if (profile && request.commandType === 'generate-sql-script' && request.output) {
    persistProfileSqlScriptOutputPath(profile.id, request.output)
  }

  interruptRequested.value = false
  running.value = true
  result.value = null
  liveExecutionOutput.value = ''
  resetExecutionOutputBuffer()

  try {
    const execution = await executeEfCommand(request)
    flushExecutionOutputBuffer()
    result.value = execution

    if (!liveExecutionOutput.value.trim()) {
      const stdout = execution.stdout || ''
      const stderr = execution.stderr || ''
      liveExecutionOutput.value = stdout && stderr ? `${stdout}\n${stderr}` : stdout || stderr
    }

    historyStore.addRecord({
      profileId: request.profileId,
      commandType: request.commandType,
      command: execution.command,
      success: execution.success,
      exitCode: execution.exitCode,
      durationMs: execution.durationMs,
      stdout: execution.stdout,
      stderr: execution.stderr
    })
    profilesStore.markUsed(request.profileId)

    if (interruptRequested.value) {
      message.warning(t('command.interrupted'))
    } else if (execution.success) {
      message.success(t('common.success'))
      void refreshProjectMetadata()
    } else {
      message.error(t('common.failed'))
    }
  } catch (error) {
    flushExecutionOutputBuffer()
    message.error(String(error))
  } finally {
    running.value = false
    interrupting.value = false
    flushExecutionOutputBuffer()
    resetExecutionOutputBuffer()
    interruptRequested.value = false
  }
}
</script>

<template>
  <n-space vertical :size="12">
    <n-h3 style="margin: 0">{{ t('command.title') }}</n-h3>

    <n-alert v-if="!selectedProfile" type="warning" :show-icon="false">
      {{ t('app.noProfile') }}
    </n-alert>

    <template v-else>
      <n-grid :cols="2" :x-gap="12">
        <n-grid-item>
          <n-form label-placement="top">
            <n-form-item :label="t('command.type')">
              <n-select v-model:value="commandType" :options="commandOptions" />
            </n-form-item>

            <n-space v-if="commandType === 'update-database' || commandType === 'generate-sql-script'" align="center" :size="8" class="metadata-refresh-row">
              <n-text v-if="loadingMetadata" depth="3" style="font-size: 12px">
                {{ t('command.loadingMetadata') }}
              </n-text>
              <n-button type="primary" :loading="loadingMetadata" :disabled="running" @click="refreshProjectMetadata(true)">
                {{ t('command.refreshMetadata') }}
              </n-button>
            </n-space>

            <n-form-item :label="t('command.context')">
              <n-select
                v-if="dbContextOptions.length"
                v-model:value="form.context"
                :options="dbContextOptions"
                filterable
                :clearable="false"
                :placeholder="t('profile.selectDbContext')"
              />
              <n-input v-else v-model:value="form.context" />
            </n-form-item>

            <n-form-item v-if="commandType === 'add-migration'">
              <template #label>
                <n-space align="center" :size="6" style="flex-wrap: wrap">
                  <span>{{ t('command.migrationName') }}</span>
                  <n-text depth="3" style="font-size: 12px">{{ t('command.migrationNameHint') }}</n-text>
                </n-space>
              </template>
              <n-space vertical :size="4" class="migration-name-block">
                <n-input
                  v-model:value="form.migrationName"
                  :placeholder="t('command.migrationNamePlaceholder')"
                  @blur="onMigrationNameBlur"
                />
                <n-text v-if="migrationFileNamePreview" depth="3" class="field-caption">
                  {{ t('command.migrationFilePreview', { fileName: migrationFileNamePreview }) }}
                </n-text>
              </n-space>
            </n-form-item>

            <n-form-item v-if="commandType === 'update-database'" :label="t('command.targetMigration')">
              <n-select
                v-if="targetMigrationOptions.length"
                v-model:value="form.targetMigration"
                :options="targetMigrationOptions"
                filterable
                clearable
                :placeholder="t('command.selectTargetMigration')"
              />
              <n-input v-else v-model:value="form.targetMigration" :placeholder="t('command.selectTargetMigration')" />
            </n-form-item>

            <template v-if="commandType === 'generate-sql-script'">
              <n-form-item :label="t('command.fromMigration')">
                <n-select
                  v-if="fromMigrationOptions.length"
                  v-model:value="form.fromMigration"
                  :options="fromMigrationOptions"
                  filterable
                  clearable
                  :placeholder="t('command.selectFromMigration')"
                />
                <n-input v-else v-model:value="form.fromMigration" :placeholder="t('command.selectFromMigration')" />
              </n-form-item>

              <n-form-item :label="t('command.toMigration')">
                <n-select
                  v-if="toMigrationOptions.length"
                  v-model:value="form.toMigration"
                  :options="toMigrationOptions"
                  filterable
                  clearable
                  :placeholder="t('command.selectToMigration')"
                />
                <n-input v-else v-model:value="form.toMigration" :placeholder="t('command.selectToMigration')" />
              </n-form-item>

              <n-form-item :label="t('command.output')">
                <div class="path-row">
                  <n-input
                    v-model:value="form.output"
                    class="path-field"
                    @blur="onSqlScriptOutputBlur"
                  />
                  <n-button class="path-btn" @click="chooseOutputPath">{{ t('command.chooseOutputFile') }}</n-button>
                </div>
              </n-form-item>
            </template>

            <n-form-item v-if="commandType === 'add-migration'" :label="t('command.outputDir')">
              <div class="path-row">
                <n-input v-model:value="form.outputDir" class="path-field" />
                <n-button class="path-btn" @click="chooseOutputDirectory">
                  {{ t('command.chooseOutputDirectory') }}
                </n-button>
              </div>
            </n-form-item>

            <n-space>
              <n-checkbox v-model:checked="form.noBuild">{{ t('command.noBuild') }}</n-checkbox>
              <n-checkbox v-model:checked="form.verbose">{{ t('command.verbose') }}</n-checkbox>
              <n-checkbox v-model:checked="form.force">{{ t('command.force') }}</n-checkbox>
              <n-checkbox v-model:checked="form.dryRun">{{ t('command.dryRun') }}</n-checkbox>
            </n-space>

            <n-space v-if="commandType === 'generate-sql-script'" style="margin-top: 8px">
              <n-checkbox v-model:checked="form.idempotent">{{ t('command.idempotent') }}</n-checkbox>
              <n-checkbox v-model:checked="form.noTransactions">{{ t('command.noTransactions') }}</n-checkbox>
            </n-space>

            <n-collapse v-model:expanded-names="advancedExpandedNames" style="margin-top: 10px">
              <n-collapse-item name="advanced" :title="t('command.advancedOptions')">
                <n-form-item>
                  <n-space align="center" :size="8">
                    <n-switch v-model:value="useConnectionOverride" />
                    <n-text>{{ t('command.overrideConnection') }}</n-text>
                  </n-space>
                </n-form-item>

                <n-form-item v-if="useConnectionOverride" :label="t('command.connection')">
                  <n-input v-model:value="form.connection" type="password" show-password-on="click" />
                </n-form-item>

                <n-form-item v-if="shouldShowFrameworkSelector" :label="t('command.framework')">
                  <n-select
                    v-model:value="form.framework"
                    :options="frameworkOptions"
                    :clearable="false"
                    :placeholder="t('command.selectFramework')"
                  />
                </n-form-item>

                <n-form-item :label="t('command.configuration')">
                  <n-select
                    v-model:value="form.configuration"
                    :options="configurationOptions"
                    :placeholder="t('command.configurationDefault')"
                  />
                </n-form-item>
              </n-collapse-item>
            </n-collapse>

            <n-space>
              <n-button @click="doPreview">{{ t('command.preview') }}</n-button>
              <n-button type="primary" :loading="running" @click="doExecute">{{ t('command.execute') }}</n-button>
              <n-button
                v-if="running"
                type="warning"
                ghost
                :loading="interrupting"
                @click="doInterrupt"
              >
                {{ t('command.interrupt') }}
              </n-button>
            </n-space>
          </n-form>
        </n-grid-item>

        <n-grid-item>
          <n-space vertical :size="12">
            <n-card :title="t('command.generatedPreview')" size="small">
              <template #header-extra>
                <n-button size="tiny" secondary :disabled="!preview?.command" @click="copyPreviewCommand">
                  {{ t('command.copyPreview') }}
                </n-button>
              </template>
              <n-input
                :value="preview?.command || ''"
                type="textarea"
                :autosize="{ minRows: 4, maxRows: 12 }"
                readonly
              />
            </n-card>

            <n-card :title="t('command.executionResult')" size="small">
              <n-space vertical :size="8">
                <n-text v-if="result" :type="result.success ? 'success' : 'error'">
                  {{ result.success ? t('common.success') : t('common.failed') }}
                  (exit: {{ result.exitCode }}, {{ result.durationMs }}ms)
                </n-text>
                <div ref="executionOutputContainerRef" class="execution-output-wrap">
                  <n-input
                    :value="executionOutputText"
                    type="textarea"
                    :autosize="false"
                    :rows="14"
                    readonly
                  />
                </div>
              </n-space>
            </n-card>
          </n-space>
        </n-grid-item>
      </n-grid>
    </template>
  </n-space>
</template>

<style scoped>
.metadata-refresh-row {
  margin-top: -2px;
  margin-bottom: 12px;
}

.migration-name-block {
  width: 100%;
}

.execution-output-wrap {
  width: 100%;
}

.field-caption {
  display: block;
  margin-top: 0;
  font-size: 12px;
  line-height: 1.4;
}

.path-row {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
}

.path-field {
  flex: 1;
  min-width: 0;
}

.path-btn {
  flex: 0 0 auto;
  white-space: nowrap;
}

:deep(.path-field .n-input) {
  width: 100%;
}
</style>
