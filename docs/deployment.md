# 部署与升级

## 支持范围

MeloArk v0.1 首版只发布 `linux/amd64` 单镜像。镜像内包含 Web、Rust Server、FFmpeg/ffprobe、Chromaprint/fpcalc 和 CA 证书；SQLite 必须放在本地持久化目录 `/data`，不要放在 SMB/NFS 音乐目录。

## 首次启动

从源码构建：

```bash
cp .env.example .env
# 生成随机 Secret，例如：openssl rand -hex 32
docker compose up -d --build
docker compose logs -f meloark
```

使用 GitHub Release 对应的 GHCR 镜像时，把 `.env` 中的镜像改为实际仓库和版本：

```bash
MELOARK_IMAGE=ghcr.io/OWNER/melo-ark:0.1.0
docker compose pull
docker compose up -d --no-build
docker compose logs -f meloark
```

Tag workflow 会发布 `linux/amd64` 镜像，并创建附带 `compose.yaml`、`.env.example`、许可证和第三方声明的 GitHub Release。正式升级应固定版本号，不要依赖 `latest` 回滚。

打开 `http://HOST:31000` 创建唯一管理员。宿主机音乐目录必须先通过 Compose 挂载；Web 只填写容器路径，例如 `/music/source`。

生产环境必须替换 `MELOARK_JWT_SECRET`。该值也用于加密本地 OpenSubsonic 凭据，修改后既有 JWT 和 OpenSubsonic 密文都会失效。

## 挂载策略

- 只想整理索引、不写源文件：使用 [`compose.read-only-source.yaml`](../examples/compose/compose.read-only-source.yaml)。
- 多来源曲库：使用 [`compose.multiple-roots.yaml`](../examples/compose/compose.multiple-roots.yaml)，然后在 Web 中逐一添加容器路径。
- Organizer 默认 Hardlink，源与目标必须位于同一文件系统；失败时不会自动复制。

## 升级

```bash
docker compose pull
docker compose up -d
docker compose logs --tail=100 meloark
```

升级前先按备份文档保存 `/data`。数据库迁移在服务启动时自动执行；不要在旧版本仍运行时让新旧容器共享同一个数据库。

## 健康检查

```bash
curl --fail http://127.0.0.1:31000/api/health
docker inspect --format '{{json .State.Health}}' meloark
```

Swagger UI 位于 `/docs/`，OpenAPI JSON 位于 `/openapi.json`。
