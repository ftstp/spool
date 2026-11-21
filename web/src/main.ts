import { mount, unmount } from 'svelte'
import '@fontsource/ibm-plex-sans/latin-400.css'
import '@fontsource/ibm-plex-sans/latin-500.css'
import '@fontsource/ibm-plex-sans/latin-600.css'
import '@fontsource/ibm-plex-mono/latin-500.css'
import '@fontsource/ibm-plex-mono/latin-600.css'
import './app.css'
import App from './App.svelte'
import Boot from './Boot.svelte'
import { i18n } from './lib/i18n/index.svelte'
import { theme } from './lib/theme.svelte'

// the page says its language (screen readers, spelling, hyphenation) from the start
document.documentElement.lang = i18n.lang
// light or dark as chosen, before the first paint
theme.init()

// the start screen first: the pages mount once the core has opened the base (they all ask
// it something), so a long open says what it does instead of showing nothing
const target = document.getElementById('app')!
const boot = mount(Boot, {
  target,
  props: {
    onready: () => {
      unmount(boot)
      mount(App, { target })
    },
  },
})
