import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

const host = process.env.TAURI_DEV_HOST

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    svelte({
      // Required for proper CSS HMR
      emitCss: true
    })
  ],

  // Vite options tailored for Tauri development
  clearScreen: false,
  root: '.', // Explicitly set root to project directory
  
  server: {
    port: 5173,
    strictPort: true,
    host: host || 'localhost',
    hmr: {
      protocol: 'ws',
      host: host || 'localhost',
      port: 5173,
    },
    watch: {
      // Ignore src-tauri to prevent build loops
      ignored: ['**/src-tauri/**', '**/node_modules/**'],
      // Use polling for more reliable file change detection (required for some macOS setups)
      usePolling: true,
      interval: 300,
      binaryInterval: 300,
      // Wait for file writes to complete
      awaitWriteFinish: {
        stabilityThreshold: 150,
        pollInterval: 100,
      },
    },
  },

  // Environment variables
  envPrefix: ['VITE_', 'TAURI_ENV_*'],

  build: {
    target: process.env.TAURI_ENV_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
}))
