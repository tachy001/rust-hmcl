import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
export default defineConfig({
  root: 'frontend',
  plugins: [vue()],
  publicDir: '../assets',
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: '../web-dist', emptyOutDir: true, target: 'es2022' },
});
