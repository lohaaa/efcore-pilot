import { createI18n } from 'vue-i18n'
import enUS from './en-US'
import zhCN from './zh-CN'

export const SUPPORT_LOCALES = ['en-US', 'zh-CN'] as const
export type SupportLocale = (typeof SUPPORT_LOCALES)[number]

const defaultLocale = (localStorage.getItem('efp_locale') as SupportLocale) || 'zh-CN'

export const i18n = createI18n({
  legacy: false,
  locale: defaultLocale,
  fallbackLocale: 'en-US',
  messages: {
    'en-US': enUS,
    'zh-CN': zhCN
  }
})

export function setLocale(locale: SupportLocale) {
  i18n.global.locale.value = locale
  localStorage.setItem('efp_locale', locale)
}
