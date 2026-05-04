import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { internalIpV4 } from "internal-ip";
import { resolve } from "path";
import { ViteEjsPlugin } from "vite-plugin-ejs";
import monacoEditorPlugin from "vite-plugin-monaco-editor";
const mobile = !!/android|ios/.exec(process.env.TAURI_ENV_PLATFORM);

// https://vitejs.dev/config/
export default defineConfig(async ({ command }) => {
  return {
    plugins: [
      ViteEjsPlugin({ debug: command == "serve" }),
      react({
        babel: {
          plugins: ["babel-plugin-react-compiler"],
        },
      }),
      (monacoEditorPlugin as any).default({}),
    ],
    build: {
      target: "esnext",
      sourcemap: true,
      rollupOptions: {
        input: {
          main: resolve(__dirname, "index.html"),
          query: resolve(__dirname, "query.html"),
        },

        output: {
          manualChunks: (id) => {
            if (id.includes("monaco-editor")) return "monaco";
          },
        },
      },
    },
    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: mobile ? "0.0.0.0" : false,
      hmr: mobile
        ? {
            protocol: "ws",
            host: await internalIpV4(),
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**", "**/node_modules/**", "**/dist/**"],
      },
    },
  };
});
