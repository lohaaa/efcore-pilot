<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { detectEnvironment } from '../tauri-api'
import type { EnvironmentStatus } from '../types/command'

const { t } = useI18n()
const loading = ref(false)
const status = ref<EnvironmentStatus | null>(null)

async function runCheck() {
  loading.value = true
  try {
    status.value = await detectEnvironment()
  } catch (error) {
    const output = String(error)
    status.value = {
      dotnet: { available: false, output },
      dotnetEf: { available: false, output }
    }
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  void runCheck()
})
</script>

<template>
  <n-space vertical :size="8">
    <n-space justify="space-between" align="center">
      <n-h3 style="margin: 0">{{ t('app.environment') }}</n-h3>
      <n-button size="small" :loading="loading" @click="runCheck">{{ t('app.runCheck') }}</n-button>
    </n-space>

    <n-card size="small" embedded>
      <n-space vertical :size="4">
        <n-text strong>{{ t('environment.dotnet') }}</n-text>
        <n-tag :type="status?.dotnet.available ? 'success' : 'error'" size="small">
          {{ status?.dotnet.available ? t('environment.available') : t('environment.unavailable') }}
        </n-tag>
        <n-ellipsis :line-clamp="2">{{ status?.dotnet.output || '-' }}</n-ellipsis>
      </n-space>
    </n-card>

    <n-card size="small" embedded>
      <n-space vertical :size="4">
        <n-text strong>{{ t('environment.dotnetEf') }}</n-text>
        <n-tag :type="status?.dotnetEf.available ? 'success' : 'error'" size="small">
          {{ status?.dotnetEf.available ? t('environment.available') : t('environment.unavailable') }}
        </n-tag>
        <n-ellipsis :line-clamp="2">{{ status?.dotnetEf.output || '-' }}</n-ellipsis>
      </n-space>
    </n-card>
  </n-space>
</template>
