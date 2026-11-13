import { mount } from 'svelte'
import '@fontsource/ibm-plex-sans/latin-400.css'
import '@fontsource/ibm-plex-sans/latin-500.css'
import '@fontsource/ibm-plex-sans/latin-600.css'
import '@fontsource/ibm-plex-mono/latin-500.css'
import '@fontsource/ibm-plex-mono/latin-600.css'
import './app.css'
import App from './App.svelte'
import { i18n } from './lib/i18n/index.svelte'
import { theme } from './lib/theme.svelte'

// the page says its language (screen readers, spelling, hyphenation) from the start
document.documentElement.lang = i18n.lang
// light or dark as chosen, before the first paint
theme.init()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
