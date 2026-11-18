import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri expects a fixed port; the brand kit lives in /assets (outside src) and is
// imported directly so replacing a file there updates the whole app.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1428, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_'],
  build: { target: 'es2022', sourcemap: false },
});
