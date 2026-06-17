import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Assets are referenced absolutely from root (/assets/...) so the same index.html
// works under any /s/:key path; the backend serves /assets/* from web/dist.
// Backend to proxy API calls to during `npm run dev`.
// Override with: BACKEND=http://your-backend:3000 npm run dev
const BACKEND = process.env.BACKEND || 'http://localhost:3000';

export default defineConfig({
  base: '/',
  plugins: [svelte()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    host: true, // listen on 0.0.0.0 so you can open it from other devices on the LAN
    proxy: {
      // SPA fetches /api/... same-origin; forward those to the real backend
      // (which has the synced album + marks/notes). The /share/:key route itself
      // is served by Vite's SPA fallback (index.html).
      '/api': { target: BACKEND, changeOrigin: true },
      '/admin': { target: BACKEND, changeOrigin: true },
    },
  },
});
