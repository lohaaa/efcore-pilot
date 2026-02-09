import { createApp } from 'vue'
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NCode,
  NConfigProvider,
  NDataTable,
  NDialogProvider,
  NEl,
  NEmpty,
  NEllipsis,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NH2,
  NH3,
  NInput,
  NLayout,
  NLayoutContent,
  NLayoutHeader,
  NLayoutSider,
  NMessageProvider,
  NModal,
  NSelect,
  NSpace,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  NText
} from 'naive-ui'
import { createPinia } from 'pinia'
import App from './App.vue'
import { i18n } from './i18n'
import './style.css'

const app = createApp(App)

const components = {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NCode,
  NConfigProvider,
  NDataTable,
  NDialogProvider,
  NEl,
  NEmpty,
  NEllipsis,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NH2,
  NH3,
  NInput,
  NLayout,
  NLayoutContent,
  NLayoutHeader,
  NLayoutSider,
  NMessageProvider,
  NModal,
  NSelect,
  NSpace,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  NText
}

Object.entries(components).forEach(([name, component]) => {
  app.component(name, component)
})

app.use(createPinia())
app.use(i18n)
app.mount('#app')
