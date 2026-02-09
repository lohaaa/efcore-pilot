<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useHistoryStore } from '../stores/history'

const { t } = useI18n()
const historyStore = useHistoryStore()

const MAX_OUTPUT_PREVIEW_CHARS = 20_000

const rows = computed(() => historyStore.records)
const expandedOutputIds = ref<string[]>([])

function isOutputExpanded(id: string) {
  return expandedOutputIds.value.includes(id)
}

function toggleOutput(id: string) {
  if (isOutputExpanded(id)) {
    expandedOutputIds.value = expandedOutputIds.value.filter((item) => item !== id)
    return
  }

  expandedOutputIds.value = [...expandedOutputIds.value, id]
}

function getOutputPreview(stdout: string, stderr: string) {
  const merged = `${stdout || ''}
${stderr || ''}`.trim()
  if (!merged) {
    return '-'
  }

  if (merged.length <= MAX_OUTPUT_PREVIEW_CHARS) {
    return merged
  }

  const tail = merged.slice(-MAX_OUTPUT_PREVIEW_CHARS)
  return `${t('history.outputTruncated')}
${tail}`
}
</script>

<template>
  <n-space vertical :size="8">
    <n-space justify="space-between" align="center">
      <n-h3 style="margin: 0">{{ t('history.title') }}</n-h3>
      <n-button quaternary size="small" @click="historyStore.clearAll">
        {{ t('history.clear') }}
      </n-button>
    </n-space>

    <n-empty v-if="!rows.length" :description="t('history.noData')" />

    <n-space v-else vertical :size="8">
      <n-card v-for="row in rows.slice(0, 20)" :key="row.id" size="small" embedded>
        <n-space vertical :size="6">
          <n-space justify="space-between" align="center">
            <n-text depth="3" style="font-size: 12px">{{ new Date(row.createdAt).toLocaleString() }}</n-text>
            <n-space align="center" :size="8">
              <n-tag :type="row.success ? 'success' : 'error'" size="small">
                {{ row.success ? t('common.success') : t('common.failed') }}
              </n-tag>
              <n-button text size="tiny" @click="toggleOutput(row.id)">
                {{ isOutputExpanded(row.id) ? t('history.hideOutput') : t('history.showOutput') }}
              </n-button>
            </n-space>
          </n-space>
          <n-code :code="row.command" language="bash" />
          <n-text depth="3" style="font-size: 12px">{{ row.durationMs }}ms</n-text>
          <n-input
            v-if="isOutputExpanded(row.id)"
            :value="getOutputPreview(row.stdout || '', row.stderr || '')"
            type="textarea"
            :autosize="false"
            :rows="8"
            readonly
          />
        </n-space>
      </n-card>
    </n-space>
  </n-space>
</template>
