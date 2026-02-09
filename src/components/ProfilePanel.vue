<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useMessage } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import { scanWorkspace } from '../tauri-api'
import { useProfilesStore } from '../stores/profiles'
import type { WorkspaceProjectInfo, WorkspaceScanResult } from '../types/discovery'
import type { ProfileFormModel, ProjectProfile } from '../types/profile'
import { pickDirectory, pickProjectFile, pickSolutionFile } from '../utils/dialog'

const message = useMessage()
const { t } = useI18n()
const profilesStore = useProfilesStore()

const showModal = ref(false)
const editingId = ref<string | undefined>()
const scanning = ref(false)
const scanResult = ref<WorkspaceScanResult | null>(null)

const formModel = reactive<ProfileFormModel>({
  name: '',
  group: '',
  tagsText: '',
  workspacePath: '',
  projectPath: '',
  migrationsProjectPath: '',
  startupProjectPath: '',
  migrationsDirectory: '',
  dbContext: '',
  enabled: true
})

const profiles = computed(() => profilesStore.profiles)

const startupProjectOptions = computed(() =>
  (scanResult.value?.projects || [])
    .filter((project) => project.isStartupCandidate)
    .map((project) => ({ label: formatProjectLabel(project), value: project.path }))
)

const migrationsProjectOptions = computed(() =>
  (scanResult.value?.projects || [])
    .filter((project) => project.isMigrationsCandidate)
    .map((project) => ({ label: formatProjectLabel(project), value: project.path }))
)

const selectedMigrationsProject = computed(() => {
  const migrationsPath = formModel.migrationsProjectPath || formModel.projectPath
  if (!migrationsPath) return null
  return scanResult.value?.projects.find((project) => project.path === migrationsPath) ?? null
})

const dbContextOptions = computed(() =>
  (selectedMigrationsProject.value?.dbContexts || []).map((item) => ({ label: item, value: item }))
)

const migrationDirectoryOptions = computed(() =>
  (selectedMigrationsProject.value?.migrationDirectories || []).map((item) => ({ label: item, value: item }))
)

function formatProjectLabel(project: WorkspaceProjectInfo) {
  return `${project.name} (${project.relativePath})`
}

function normalizePath(value: string) {
  return value.replace(/\\/g, '/').replace(/\/+$/, '')
}

function isAbsolutePath(value: string) {
  const normalized = value.replace(/\\/g, '/')
  return normalized.startsWith('/') || /^[a-zA-Z]:\//.test(normalized) || normalized.startsWith('//')
}

function getProjectDirectory(projectPath: string): string {
  const normalized = normalizePath(projectPath)
  const slashIndex = normalized.lastIndexOf('/')
  if (slashIndex < 0) {
    return normalized
  }
  return normalized.slice(0, slashIndex)
}

function toRelativeMigrationsDirectory(selectedDirectory: string): string {
  const migrationsProjectPath = formModel.migrationsProjectPath || formModel.projectPath
  if (!migrationsProjectPath) {
    return selectedDirectory
  }

  const projectDirectory = normalizePath(getProjectDirectory(migrationsProjectPath))
  const selected = normalizePath(selectedDirectory)

  if (selected === projectDirectory) {
    return 'Migrations'
  }

  if (selected.startsWith(`${projectDirectory}/`)) {
    return selected.slice(projectDirectory.length + 1)
  }

  return selectedDirectory
}

function getMigrationsDirectoryDefaultPath() {
  const migrationsProjectPath = (formModel.migrationsProjectPath || formModel.projectPath).trim()
  const projectDirectory = migrationsProjectPath ? getProjectDirectory(migrationsProjectPath) : ''
  const configuredDirectory = formModel.migrationsDirectory?.trim() || ''

  if (configuredDirectory) {
    if (isAbsolutePath(configuredDirectory)) {
      return configuredDirectory
    }

    if (projectDirectory) {
      const cleanRelative = configuredDirectory.replace(/^[/\\]+/, '')
      return `${normalizePath(projectDirectory)}/${cleanRelative}`
    }
  }

  if (projectDirectory) {
    return projectDirectory
  }

  return formModel.workspacePath || undefined
}

function resetForm() {
  formModel.name = ''
  formModel.group = ''
  formModel.tagsText = ''
  formModel.workspacePath = ''
  formModel.projectPath = ''
  formModel.migrationsProjectPath = ''
  formModel.startupProjectPath = ''
  formModel.migrationsDirectory = ''
  formModel.dbContext = ''
  formModel.enabled = true
  scanResult.value = null
}

function openForCreate() {
  editingId.value = undefined
  resetForm()
  showModal.value = true
}

async function loadScanForPath(path: string) {
  const trimmed = path.trim()
  if (!trimmed) return

  scanning.value = true
  try {
    const result = await scanWorkspace({ path: trimmed })
    scanResult.value = result

    if (!formModel.workspacePath) {
      formModel.workspacePath = result.solutionPath || result.workspaceRoot
    }

    if (!formModel.startupProjectPath && startupProjectOptions.value.length) {
      formModel.startupProjectPath = startupProjectOptions.value[0].value
    }

    if (!formModel.migrationsProjectPath && migrationsProjectOptions.value.length) {
      formModel.migrationsProjectPath = migrationsProjectOptions.value[0].value
      formModel.projectPath = formModel.migrationsProjectPath
    }

    syncProjectDerivedSelections()
    message.success(t('profile.scanSuccess'))
  } catch (error) {
    message.error(`${t('profile.scanFailed')}: ${String(error)}`)
  } finally {
    scanning.value = false
  }
}

function openForEdit(profile: ProjectProfile) {
  editingId.value = profile.id
  formModel.name = profile.name
  formModel.group = profile.group
  formModel.tagsText = profile.tags.join(', ')
  formModel.workspacePath = profile.workspacePath || ''
  formModel.projectPath = profile.projectPath
  formModel.migrationsProjectPath = profile.migrationsProjectPath || profile.projectPath
  formModel.startupProjectPath = profile.startupProjectPath
  formModel.migrationsDirectory = profile.migrationsDirectory || ''
  formModel.dbContext = profile.dbContext
  formModel.enabled = profile.enabled

  scanResult.value = null
  showModal.value = true

  const pathForScan = formModel.workspacePath || formModel.migrationsProjectPath || formModel.startupProjectPath
  if (pathForScan) {
    void loadScanForPath(pathForScan)
  }
}

function syncProjectDerivedSelections() {
  const project = selectedMigrationsProject.value
  if (!project) return

  if (project.dbContexts.length && !project.dbContexts.includes(formModel.dbContext || '')) {
    formModel.dbContext = project.dbContexts[0]
  }

  if (
    project.migrationDirectories.length &&
    !project.migrationDirectories.includes(formModel.migrationsDirectory || '')
  ) {
    formModel.migrationsDirectory = project.migrationDirectories[0]
  }
}

async function scanWorkspaceNow() {
  const path = formModel.workspacePath.trim()
  if (!path) {
    message.warning(t('profile.workspaceRequired'))
    return
  }
  await loadScanForPath(path)
}

function onMigrationsProjectChange(path: string | null) {
  const nextPath = path ?? ''
  formModel.migrationsProjectPath = nextPath
  formModel.projectPath = nextPath
  syncProjectDerivedSelections()
}

async function chooseWorkspaceDirectory() {
  const selected = await pickDirectory(formModel.workspacePath)
  if (!selected) return
  formModel.workspacePath = selected
  await loadScanForPath(selected)
}

async function chooseWorkspaceSolution() {
  const selected = await pickSolutionFile(formModel.workspacePath)
  if (!selected) return
  formModel.workspacePath = selected
  await loadScanForPath(selected)
}

async function chooseStartupProjectPath() {
  const selected = await pickProjectFile(formModel.startupProjectPath || formModel.workspacePath)
  if (!selected) return
  formModel.startupProjectPath = selected
}

async function chooseMigrationsProjectPath() {
  const selected = await pickProjectFile(formModel.migrationsProjectPath || formModel.workspacePath)
  if (!selected) return
  onMigrationsProjectChange(selected)
}

async function chooseMigrationsDirectory() {
  const selected = await pickDirectory(getMigrationsDirectoryDefaultPath())
  if (!selected) return
  formModel.migrationsDirectory = toRelativeMigrationsDirectory(selected)
}

function saveProfile() {
  formModel.projectPath = formModel.migrationsProjectPath || formModel.projectPath
  formModel.dbContext = formModel.dbContext.trim()

  if (!formModel.dbContext) {
    message.warning(t('profile.dbContextRequired'))
    return
  }

  if (!formModel.name || !formModel.projectPath || !formModel.startupProjectPath) {
    message.warning(t('profile.required'))
    return
  }

  profilesStore.upsertProfile({
    ...formModel,
    id: editingId.value
  })

  showModal.value = false
}

function removeProfile(profile: ProjectProfile) {
  window.setTimeout(() => {
    const confirmed = window.confirm(t('profile.removeConfirm'))
    if (!confirmed) return
    profilesStore.removeProfile(profile.id)
  }, 0)
}
</script>

<template>
  <n-space vertical :size="12">
    <n-space justify="space-between" align="center">
      <n-h3 style="margin: 0">{{ t('profile.title') }}</n-h3>
      <n-button size="small" type="primary" @click="openForCreate">
        {{ t('profile.add') }}
      </n-button>
    </n-space>

    <n-empty v-if="!profiles.length" :description="t('profile.empty')" />

    <n-space v-else vertical :size="8">
      <n-card
        v-for="profile in profiles"
        :key="profile.id"
        size="small"
        :class="{ selected: profilesStore.selectedProfileId === profile.id }"
        embedded
        @click="profilesStore.selectProfile(profile.id)"
      >
        <n-space vertical :size="6">
          <n-text strong>{{ profile.name }}</n-text>
          <n-text depth="3" style="font-size: 12px">{{ profile.migrationsProjectPath || profile.projectPath }}</n-text>
          <n-space justify="space-between" align="center">
            <n-tag size="small" :type="profile.enabled ? 'success' : 'default'">
              {{ profile.enabled ? t('environment.available') : t('environment.unavailable') }}
            </n-tag>
            <n-space :size="6">
              <n-button size="tiny" tertiary @click.stop="openForEdit(profile)">
                {{ t('profile.edit') }}
              </n-button>
              <n-button size="tiny" tertiary type="error" @click.stop="removeProfile(profile)">
                {{ t('profile.delete') }}
              </n-button>
            </n-space>
          </n-space>
        </n-space>
      </n-card>
    </n-space>

    <n-modal v-model:show="showModal" preset="card" style="width: 760px">
      <template #header>
        {{ editingId ? t('profile.edit') : t('profile.add') }}
      </template>

      <n-form label-placement="top">
        <n-form-item :label="t('profile.name')">
          <n-input v-model:value="formModel.name" />
        </n-form-item>
        <n-form-item :label="t('profile.group')">
          <n-input v-model:value="formModel.group" />
        </n-form-item>
        <n-form-item :label="t('profile.tags')">
          <n-input v-model:value="formModel.tagsText" />
        </n-form-item>

        <n-form-item :label="t('profile.workspacePath')">
          <div class="path-row">
            <n-input
              v-model:value="formModel.workspacePath"
              class="path-field"
              :placeholder="t('profile.workspacePathPlaceholder')"
            />
            <n-button class="path-btn" @click="chooseWorkspaceDirectory">
              {{ t('profile.chooseDirectory') }}
            </n-button>
            <n-button class="path-btn" @click="chooseWorkspaceSolution">
              {{ t('profile.chooseSolutionFile') }}
            </n-button>
            <n-button class="path-btn" :loading="scanning" @click="scanWorkspaceNow">
              {{ t('profile.scanWorkspace') }}
            </n-button>
          </div>
        </n-form-item>

        <n-form-item :label="t('profile.startupProjectPath')">
          <div class="path-row">
            <n-select
              v-if="startupProjectOptions.length"
              v-model:value="formModel.startupProjectPath"
              class="path-field"
              :options="startupProjectOptions"
              filterable
              clearable
              :placeholder="t('profile.selectStartupProject')"
            />
            <n-input v-else v-model:value="formModel.startupProjectPath" class="path-field" />
            <n-button class="path-btn" @click="chooseStartupProjectPath">
              {{ t('profile.chooseProjectFile') }}
            </n-button>
          </div>
        </n-form-item>

        <n-form-item :label="t('profile.migrationsProjectPath')">
          <div class="path-row">
            <n-select
              v-if="migrationsProjectOptions.length"
              :value="formModel.migrationsProjectPath"
              class="path-field"
              :options="migrationsProjectOptions"
              filterable
              clearable
              :placeholder="t('profile.selectMigrationsProject')"
              @update:value="onMigrationsProjectChange"
            />
            <n-input
              v-else
              :value="formModel.migrationsProjectPath"
              class="path-field"
              @update:value="onMigrationsProjectChange"
            />
            <n-button class="path-btn" @click="chooseMigrationsProjectPath">
              {{ t('profile.chooseProjectFile') }}
            </n-button>
          </div>
        </n-form-item>

        <n-form-item :label="t('profile.dbContext')">
          <n-select
            v-if="dbContextOptions.length"
            v-model:value="formModel.dbContext"
            :options="dbContextOptions"
            filterable
            :placeholder="t('profile.selectDbContext')"
          />
          <n-input v-else v-model:value="formModel.dbContext" />
        </n-form-item>

        <n-form-item :label="t('profile.migrationsDirectory')">
          <div class="path-row">
            <n-select
              v-if="migrationDirectoryOptions.length"
              v-model:value="formModel.migrationsDirectory"
              class="path-field"
              :options="migrationDirectoryOptions"
              filterable
              clearable
              :placeholder="t('profile.selectMigrationsDirectory')"
            />
            <n-input v-else v-model:value="formModel.migrationsDirectory" class="path-field" />
            <n-button class="path-btn" @click="chooseMigrationsDirectory">
              {{ t('profile.chooseDirectory') }}
            </n-button>
          </div>
        </n-form-item>

        <n-form-item :label="t('profile.enabled')">
          <n-switch v-model:value="formModel.enabled" />
        </n-form-item>
      </n-form>

      <template #footer>
        <n-space justify="end">
          <n-button @click="showModal = false">{{ t('profile.cancel') }}</n-button>
          <n-button type="primary" @click="saveProfile">{{ t('profile.save') }}</n-button>
        </n-space>
      </template>
    </n-modal>
  </n-space>
</template>

<style scoped>
.selected {
  border-color: var(--n-color-target);
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

:deep(.path-field .n-input),
:deep(.path-field .n-base-selection) {
  width: 100%;
}
</style>
