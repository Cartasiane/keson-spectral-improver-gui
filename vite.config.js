import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// Vite config tuned for Tauri (file protocol + relative assets)
export default defineConfig(({ mode }) => ({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true
  },
  build: {
    target: process.env.TAURI_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: mode === 'production'
  }
}))
