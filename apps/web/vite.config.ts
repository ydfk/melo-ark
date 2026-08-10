/*
 * @Description: Copyright (c) ydfk. All rights reserved
 * @Author: ydfk
 * @Date: 2026-02-02 20:49:22
 * @LastEditors: ydfk
 * @LastEditTime: 2026-02-02 22:03:13
 */
/// <reference types="vitest" />
import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  // 加载当前模式下的环境变量
  const env = loadEnv(mode, process.cwd());
  return {
    plugins: [react()],
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    server: {
      port: Number(env.VITE_PORT || 5173),
      strictPort: true,
      proxy: {
        "/api": {
          target: env.VITE_PROXY_HOST || "http://127.0.0.1:31000",
          changeOrigin: true,
        },
      },
    },
    test: {
      environment: "jsdom",
      globals: true,
      exclude: ["**/node_modules/**", "**/.git/**", "**/.worktrees/**"],
      passWithNoTests: true,
      setupFiles: ["src/test/setup.ts"],
    },
  };
});
