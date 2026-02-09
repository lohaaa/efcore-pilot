import { defineStore } from 'pinia'
import type { HistoryRecord } from '../types/command'

const STORAGE_KEY = 'efp_history_v1'

function readHistory(): HistoryRecord[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as HistoryRecord[]
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function writeHistory(history: HistoryRecord[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(history.slice(0, 200)))
}

function makeId() {
  return crypto.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
}

export const useHistoryStore = defineStore('history', {
  state: () => ({
    records: readHistory()
  }),
  actions: {
    addRecord(record: Omit<HistoryRecord, 'id' | 'createdAt'>) {
      const next: HistoryRecord = {
        ...record,
        id: makeId(),
        createdAt: new Date().toISOString()
      }
      this.records.unshift(next)
      writeHistory(this.records)
    },
    clearAll() {
      this.records = []
      writeHistory(this.records)
    }
  }
})
