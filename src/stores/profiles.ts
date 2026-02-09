import { defineStore } from 'pinia'
import type { ProfileFormModel, ProjectProfile } from '../types/profile'

const STORAGE_KEY = 'efp_profiles_v1'
const SELECTED_KEY = 'efp_selected_profile_v1'

type StoredProfile = Partial<ProjectProfile> & {
  id: string
  name: string
  tags?: string[]
  enabled?: boolean
  createdAt?: string
  updatedAt?: string
}

function normalizeStoredProfile(raw: StoredProfile): ProjectProfile {
  const fallbackProjectPath = (raw.projectPath ?? raw.migrationsProjectPath ?? '').trim()
  const workspacePath = (raw.workspacePath ?? '').trim() || fallbackProjectPath
  const migrationsProjectPath = (raw.migrationsProjectPath ?? raw.projectPath ?? '').trim()
  const startupProjectPath = (raw.startupProjectPath ?? '').trim()
  const updatedAt = raw.updatedAt ?? raw.createdAt ?? nowIso()

  return {
    id: raw.id,
    name: (raw.name ?? '').trim(),
    group: raw.group?.trim(),
    tags: Array.isArray(raw.tags) ? raw.tags.map((item) => String(item).trim()).filter(Boolean) : [],
    workspacePath,
    projectPath: migrationsProjectPath,
    migrationsProjectPath,
    startupProjectPath,
    migrationsDirectory: raw.migrationsDirectory?.trim(),
    dbContext: raw.dbContext?.trim() ?? '',
    createdAt: raw.createdAt ?? updatedAt,
    updatedAt,
    lastUsedAt: raw.lastUsedAt,
    enabled: raw.enabled ?? true
  }
}

function readProfiles(): ProjectProfile[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as StoredProfile[]
    if (!Array.isArray(parsed)) return []
    return parsed
      .filter((item): item is StoredProfile => !!item && typeof item.id === 'string')
      .map((item) => normalizeStoredProfile(item))
      .filter((item) => item.name && item.migrationsProjectPath && item.startupProjectPath)
  } catch {
    return []
  }
}

function readSelectedProfileId(): string {
  return localStorage.getItem(SELECTED_KEY) ?? ''
}

function writeProfiles(profiles: ProjectProfile[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(profiles))
}

function writeSelectedProfileId(profileId: string) {
  localStorage.setItem(SELECTED_KEY, profileId)
}

function nowIso() {
  return new Date().toISOString()
}

function makeId() {
  return crypto.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
}

const initialProfiles = readProfiles()
const initialSelectedProfileId = (() => {
  const saved = readSelectedProfileId()
  if (saved && initialProfiles.some((item) => item.id === saved)) {
    return saved
  }
  return initialProfiles[0]?.id ?? ''
})()

export const useProfilesStore = defineStore('profiles', {
  state: () => ({
    profiles: initialProfiles,
    selectedProfileId: initialSelectedProfileId
  }),
  getters: {
    selectedProfile(state): ProjectProfile | null {
      return state.profiles.find((profile) => profile.id === state.selectedProfileId) ?? null
    }
  },
  actions: {
    selectProfile(id: string) {
      this.selectedProfileId = id
      writeSelectedProfileId(id)
    },
    upsertProfile(form: ProfileFormModel) {
      const workspacePath = form.workspacePath.trim()
      const migrationsProjectPath = (form.migrationsProjectPath || form.projectPath).trim()
      const startupProjectPath = form.startupProjectPath.trim()
      const dbContext = form.dbContext.trim()

      if (!form.name || !migrationsProjectPath || !startupProjectPath || !dbContext) {
        return
      }

      const tags = form.tagsText
        .split(',')
        .map((item) => item.trim())
        .filter(Boolean)

      const updatedAt = nowIso()
      if (form.id) {
        const index = this.profiles.findIndex((item) => item.id === form.id)
        if (index >= 0) {
          this.profiles[index] = {
            ...this.profiles[index],
            name: form.name,
            group: form.group?.trim(),
            tags,
            workspacePath,
            projectPath: migrationsProjectPath,
            migrationsProjectPath,
            startupProjectPath,
            migrationsDirectory: form.migrationsDirectory?.trim(),
            dbContext,
            enabled: form.enabled,
            updatedAt
          }
        }
      } else {
        const profile: ProjectProfile = {
          id: makeId(),
          name: form.name,
          group: form.group?.trim(),
          tags,
          workspacePath,
          projectPath: migrationsProjectPath,
          migrationsProjectPath,
          startupProjectPath,
          migrationsDirectory: form.migrationsDirectory?.trim(),
          dbContext,
          createdAt: updatedAt,
          updatedAt,
          enabled: form.enabled
        }
        this.profiles.unshift(profile)
        this.selectedProfileId = profile.id
        writeSelectedProfileId(profile.id)
      }
      writeProfiles(this.profiles)
    },
    removeProfile(id: string) {
      this.profiles = this.profiles.filter((profile) => profile.id !== id)
      if (this.selectedProfileId === id) {
        this.selectedProfileId = this.profiles[0]?.id ?? ''
        writeSelectedProfileId(this.selectedProfileId)
      }
      writeProfiles(this.profiles)
    },
    markUsed(id: string) {
      const profile = this.profiles.find((item) => item.id === id)
      if (!profile) return
      profile.lastUsedAt = nowIso()
      writeProfiles(this.profiles)
    }
  }
})
