import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { internalIpV4 } from 'internal-ip';
import { resolve } from 'path';

const mobile = !!/android|ios/.exec(process.env.TAURI_ENV_PLATFORM);

// https://vitejs.dev/config/
export default defineConfig(async ({ command }) => {
  return {
    plugins: [
      react(),
    ],
    build: {
      target: 'esnext',
      rollupOptions: {
        input: {
          main: resolve(__dirname, 'index.html'),
          query: resolve(__dirname, 'query.html'),
          table_view: resolve(__dirname, 'table_view.html'),
          setting: resolve(__dirname, 'setting.html'),
          add_connection: resolve(__dirname, 'add_connection.html'),
          log_windows: resolve(__dirname, 'log_window.html'),
          info: resolve(__dirname, 'info.html'),
          query_response: resolve(__dirname, 'query_response.html'),
          table_add: resolve(__dirname, 'table_add.html'),
        }
      }
    },
    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: mobile ? '0.0.0.0' : false,
      hmr: mobile
        ? {
          protocol: 'ws',
          host: await internalIpV4(),
          port: 1421,
        }
        : undefined,
      watch: {
        // 3. tell vite to ignore watching `src-tauri`
        ignored: ['**/src-tauri/**'],
      },
    },
  };
});
