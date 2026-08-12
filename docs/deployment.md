# 部署与升级

## 支持范围

MeloArk v0.1 仅发布 `linux/amd64` 镜像。镜像使用 Alpine Linux 运行时，内置 Web、Rust Server、FFmpeg/ffprobe、Chromaprint/fpcalc 和 CA 证书；SQLite 必须放在本地持久化目录 `/data`，不要放在 SMB/NFS 音乐目录。

## 首次启动

从源码构建：

```bash
cp .env.example .env
# 生成随机 Secret，例如：openssl rand -hex 32
docker compose up -d --build
docker compose logs -f meloark
```

生产环境只拉取已经发布的版本镜像，不在服务器上现场编译：

```bash
cp .env.production.example .env.production
sudo install -d -m 0750 -o 10001 -g 10001 /srv/meloark/data /mnt/music-managed
sudo mkdir -p /mnt/music
openssl rand -hex 32
# 将输出写入 .env.production 的 MELOARK_JWT_SECRET，并核对三个宿主机目录
docker compose --env-file .env.production -f compose.production.yaml config --quiet
docker compose --env-file .env.production -f compose.production.yaml pull
docker compose --env-file .env.production -f compose.production.yaml up -d
docker compose --env-file .env.production -f compose.production.yaml ps
```

推送 `v1.2.3` 或 `v1.2.3-rc.1` 格式的 Tag 会自动触发镜像构建；也可以在 GitHub Actions 中手动运行并输入一个已存在的 Tag。Tag workflow 会检出该 Tag 对应的代码，将 `linux/amd64` 镜像推送到 Docker Hub，并创建或更新附带开发/生产 Compose、环境模板、许可证和第三方声明的 GitHub Release。仓库需要配置 `DOCKERHUB_USERNAME` 和 `DOCKERHUB_TOKEN` Secrets。正式升级应固定版本号，不要依赖 `latest` 回滚。

打开 `http://HOST:31000` 创建唯一管理员。宿主机音乐目录必须先通过 Compose 挂载；Web 只填写容器路径，例如 `/music/source`。

容器固定以 UID/GID `10001:10001` 运行。`MELOARK_DATA_PATH` 与 `MELOARK_MANAGED_PATH` 必须允许该用户写入；若要编辑源文件 Tag，`MELOARK_SOURCE_PATH` 也必须通过所有权、组权限或 ACL 授予读写权限。Web 中的整理目录路径是 `/music/organized`。

生产环境必须替换 `MELOARK_JWT_SECRET`。该值也用于加密本地 OpenSubsonic 凭据，修改后既有 JWT 和 OpenSubsonic 密文都会失效。

生产镜像已内置非 root 用户和健康检查；`compose.production.yaml` 只额外启用只读根文件系统与禁止提权。若由同机反向代理访问，可把 `MELOARK_BIND_ADDRESS` 改为 `127.0.0.1`。CPU、内存、日志等限制应按实际部署环境通过 Compose 覆盖文件设置。

## 平台与 Windows

MeloArk 仅支持 `linux/amd64`，适用于常见 Intel/AMD Linux 和 Windows Docker Desktop。不支持其他 CPU 架构或 Windows Containers 模式。

从 Mac 单独构建一个可导入 Windows Docker Desktop 的离线镜像时，只需构建 `linux/amd64`：

```bash
docker buildx build --platform linux/amd64 --load -t meloark:0.1.0-amd64 .
docker save -o meloark-0.1.0-linux-amd64.tar meloark:0.1.0-amd64
```

在 Windows Docker Desktop 切换到 Linux containers 后执行：

```powershell
docker load -i meloark-0.1.0-linux-amd64.tar
```

自行发布镜像时也应显式固定 `linux/amd64`：

```bash
docker buildx build \
  --platform linux/amd64 \
  --tag REGISTRY/OWNER/meloark:0.1.0 \
  --push .
```

## 挂载策略

- 只想整理索引、不写源文件：使用 [`compose.read-only-source.yaml`](../examples/compose/compose.read-only-source.yaml)。
- 多来源曲库：使用 [`compose.multiple-roots.yaml`](../examples/compose/compose.multiple-roots.yaml)，然后在 Web 中逐一添加容器路径。
- Organizer 默认 Hardlink，源与目标必须位于同一文件系统；失败时不会自动复制。

## 升级

```bash
docker compose --env-file .env.production -f compose.production.yaml pull
docker compose --env-file .env.production -f compose.production.yaml up -d
docker compose --env-file .env.production -f compose.production.yaml logs --tail=100 meloark
```

升级前先按备份文档保存 `/data`。数据库迁移在服务启动时自动执行；不要在旧版本仍运行时让新旧容器共享同一个数据库。

## 健康检查

```bash
curl --fail http://127.0.0.1:31000/api/health
docker compose --env-file .env.production -f compose.production.yaml ps
```

Swagger UI 位于 `/docs/`，OpenAPI JSON 位于 `/openapi.json`。
