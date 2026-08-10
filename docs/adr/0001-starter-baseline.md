# ADR 0001：保留指定 starter 基线

- 状态：已接受
- 日期：2026-08-10

## 决定

Web 基于 `ydfk/react-starter` commit `0f8a4c5933dc276d77443aebb673e2152caf2759`；Server 基于 `ydfk/rust-axum-starter` commit `ca86e9058d44732c13450d1960557c9604036d72`。

保留 React 19、TypeScript 7、Vite 8、Tailwind 4、shadcn/Radix、Zustand、Alova、Vitest，以及 Rust 2024、Axum、Tokio、SQLx SQLite、Utoipa、JWT、Argon2 与 `unsafe_code = forbid`。不通过降级依赖解决兼容问题。

## 影响

两个 starter 的独立质量脚本保留在各自应用目录。根 Dockerfile 负责把两端组合成单镜像，前端 API 默认使用同源 `/api`。
