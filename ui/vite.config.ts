import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Shared by the Tauri desktop app and the `memhub serve` browser mode.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: '0.0.0.0',
    allowedHosts: true,
    proxy: { '/api': 'http://127.0.0.1:7337' },
  },
  build: { outDir: 'dist', emptyOutDir: true, target: 'es2020' },
})
