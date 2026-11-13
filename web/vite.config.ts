import { defineConfig, type Plugin } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { readdirSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'

// public/dev-*.json hold a copy of the user's own catalogue for the browser preview: they
// must never end up in a build (the desktop app embeds dist/ in its executable).
const dropDevData = (): Plugin => {
  let outDir = 'dist'
  return {
    name: 'drop-dev-data',
    apply: 'build',
    configResolved(c) { outDir = resolve(c.root, c.build.outDir) },
    closeBundle() {
      // every dev-*.json, not a list: a new preview file must not slip into the build
      for (const f of readdirSync(outDir)) if (/^dev-.*\.json$/.test(f)) rmSync(resolve(outDir, f), { force: true })
    },
  }
}

// The desktop shell loads this exact address in development: a fixed port that fails
// loudly when taken, so the window can never show another app served on a common port.
export default defineConfig({
  plugins: [svelte(), dropDevData()],
  clearScreen: false,
  server: { port: 1430, strictPort: true, host: '127.0.0.1' },
})
