import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { internalIpV4 } from 'internal-ip';
import topLevelAwait from 'vite-plugin-top-level-await';
import fs from 'fs';

// @ts-expect-error process is a nodejs global
const mobile = !!/android|ios/.exec(process.env.TAURI_ENV_PLATFORM);

// https://vitejs.dev/config/
export default defineConfig(async ({ command }) => {
  {
    const debugFile = __dirname + '/src/comp/debug.ts';
    const content = `const debugBuild = ${command == 'serve'};
export { debugBuild }`;
    fs.writeFileSync(debugFile, content);
  }
  return {
    plugins: [
      topLevelAwait({
        // The export name of top-level await promise for each chunk module
        promiseExportName: '__tla',
        // The function to generate import names of top-level await promise in each chunk module
        promiseImportName: (i) => `__tla_${i}`,
      }),
      react(),
    ],
    build: {
      target: 'esnext',
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
