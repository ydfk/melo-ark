# MeloArk HTTP Contract

MeloArk 以 Axum 的 OpenAPI 定义为接口事实来源。运行服务后访问：

- `GET /docs`：交互式文档；
- `GET /openapi.json`：OpenAPI 3.1 JSON。

约定：应用接口统一位于 `/api`，JSON 字段使用 camelCase，时间使用 RFC 3339 UTC；除健康检查、首次初始化状态、初始化和登录外，接口均要求 `Authorization: Bearer <token>`。

当前 M0/M1 接口包括：

- `/api/health`
- `/api/auth/setup-status`、`/api/auth/setup`、`/api/auth/login`、`/api/auth/profile`
- `/api/dashboard/stats`
- `/api/libraries`、`/api/libraries/preflight`、`/api/libraries/{id}/scan`
- `/api/tracks`
- `/api/jobs` 及 pause、resume、cancel、retry-failed 动作
- `/api/events`（SSE，Bearer Header 鉴权）

错误响应使用 RFC 9457 风格的 Problem JSON。前端 DTO 位于 `apps/web/src/lib/api/types.ts`，契约和端到端验证位于 `apps/server/tests`。
