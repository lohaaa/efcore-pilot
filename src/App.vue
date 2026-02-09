<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { darkTheme, dateEnUS, dateZhCN, enUS, zhCN } from 'naive-ui'
import ProfilePanel from './components/ProfilePanel.vue'
import CommandRunner from './components/CommandRunner.vue'
import HistoryPanel from './components/HistoryPanel.vue'
import EnvironmentPanel from './components/EnvironmentPanel.vue'
import { SUPPORT_LOCALES, setLocale, type SupportLocale } from './i18n'
import { useProfilesStore } from './stores/profiles'

const THEME_MODE_STORAGE_KEY = 'efp_theme_mode'

type ThemeMode = 'auto' | 'light' | 'dark'

function readInitialThemeMode(): ThemeMode {
  const saved = localStorage.getItem(THEME_MODE_STORAGE_KEY)
  if (saved === 'auto' || saved === 'light' || saved === 'dark') {
    return saved
  }
  return 'auto'
}

const { t, locale } = useI18n()
const profilesStore = useProfilesStore()
const activeTab = ref('runner')

const selectedLocale = ref<SupportLocale>(locale.value as SupportLocale)
const selectedThemeMode = ref<ThemeMode>(readInitialThemeMode())
const isSystemDark = ref(false)

let colorSchemeMedia: MediaQueryList | null = null
let colorSchemeHandler: ((event: MediaQueryListEvent) => void) | null = null

const localeOptions = computed(() =>
  SUPPORT_LOCALES.map((item) => ({
    label: item,
    value: item
  }))
)

const themeModeOptions = computed(() => [
  { label: t('app.themeAuto'), value: 'auto' },
  { label: t('app.themeLight'), value: 'light' },
  { label: t('app.themeDark'), value: 'dark' }
])

const selectedProfileName = computed(() => profilesStore.selectedProfile?.name || '-')

const naiveLocale = computed(() => (selectedLocale.value === 'zh-CN' ? zhCN : enUS))
const naiveDateLocale = computed(() => (selectedLocale.value === 'zh-CN' ? dateZhCN : dateEnUS))

const naiveTheme = computed(() => {
  if (selectedThemeMode.value === 'dark') {
    return darkTheme
  }
  if (selectedThemeMode.value === 'light') {
    return null
  }
  return isSystemDark.value ? darkTheme : null
})

function onLocaleChange(nextLocale: SupportLocale) {
  selectedLocale.value = nextLocale
  setLocale(nextLocale)
}

function onThemeModeChange(nextMode: ThemeMode) {
  selectedThemeMode.value = nextMode
  localStorage.setItem(THEME_MODE_STORAGE_KEY, nextMode)
}

onMounted(() => {
  if (typeof window === 'undefined' || !window.matchMedia) return

  colorSchemeMedia = window.matchMedia('(prefers-color-scheme: dark)')
  isSystemDark.value = colorSchemeMedia.matches

  colorSchemeHandler = (event: MediaQueryListEvent) => {
    isSystemDark.value = event.matches
  }

  if (colorSchemeMedia.addEventListener) {
    colorSchemeMedia.addEventListener('change', colorSchemeHandler)
  } else {
    colorSchemeMedia.addListener(colorSchemeHandler)
  }
})

onBeforeUnmount(() => {
  if (!colorSchemeMedia || !colorSchemeHandler) return
  if (colorSchemeMedia.removeEventListener) {
    colorSchemeMedia.removeEventListener('change', colorSchemeHandler)
  } else {
    colorSchemeMedia.removeListener(colorSchemeHandler)
  }
})
</script>

<template>
  <n-config-provider :locale="naiveLocale" :date-locale="naiveDateLocale" :theme="naiveTheme">
    <n-message-provider>
      <n-layout class="app-layout">
        <n-layout-header bordered class="header">
          <n-space justify="space-between" align="center">
            <n-space vertical :size="2">
              <n-h2 style="margin: 0">{{ t('app.title') }}</n-h2>
              <n-text depth="3">{{ t('app.subtitle') }}</n-text>
            </n-space>

            <n-space align="center" :size="14">
              <n-space align="center" :size="6">
                <n-text depth="3">{{ t('app.language') }}</n-text>
                <n-select
                  style="width: 120px"
                  size="small"
                  :value="selectedLocale"
                  :options="localeOptions"
                  @update:value="onLocaleChange"
                />
              </n-space>

              <n-space align="center" :size="6">
                <n-text depth="3">{{ t('app.theme') }}</n-text>
                <n-select
                  style="width: 120px"
                  size="small"
                  :value="selectedThemeMode"
                  :options="themeModeOptions"
                  @update:value="onThemeModeChange"
                />
              </n-space>
            </n-space>
          </n-space>
        </n-layout-header>

        <n-layout has-sider class="body-layout">
          <n-layout-sider bordered width="360" content-style="padding: 12px; overflow: auto;">
            <n-space vertical :size="16">
              <ProfilePanel />
              <EnvironmentPanel />
            </n-space>
          </n-layout-sider>

          <n-layout-content content-style="padding: 12px; overflow: auto;">
            <n-space vertical :size="12">
              <n-card size="small" embedded>
                <n-text depth="3">{{ t('app.selectedProfile') }}：</n-text>
                <n-text strong>{{ selectedProfileName }}</n-text>
              </n-card>

              <n-tabs v-model:value="activeTab" type="line" :animated="false">
                <n-tab-pane name="runner" :tab="t('command.title')">
                  <CommandRunner />
                </n-tab-pane>
                <n-tab-pane name="history" :tab="t('history.title')">
                  <HistoryPanel />
                </n-tab-pane>
              </n-tabs>
            </n-space>
          </n-layout-content>
        </n-layout>
      </n-layout>
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
.app-layout {
  height: 100vh;
}

.header {
  padding: 12px 16px;
}

.body-layout {
  height: calc(100vh - 74px);
}
</style>
