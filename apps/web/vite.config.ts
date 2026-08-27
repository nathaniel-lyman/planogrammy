import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: { environment: 'jsdom', exclude: ['tests/browser/**', 'node_modules/**'] },
  server: { port: 4173, strictPort: true },
});
