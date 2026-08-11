# MeloArk

![MeloArk Wordmark](apps/web/public/meloark-wordmark.svg)

MeloArk 是面向 NAS / HomeLab 的自托管本地音乐管理、整理与播放服务。它索引用户已经挂载的音乐文件，不提供在线音乐下载。

## 能力

- 多曲库增量扫描、NAS Watch + 周期 reconciliation、SQLite FTS5 中英文/全拼/拼音首字母搜索；
- 单曲/批量 Tag、封面、快照、Preview → Apply、撤销与失败重试；
- QQ、网易、酷狗、MusicBrainz 稳定元数据候选，默认关闭的 Kuwo/Migu Beta 适配器，以及同步歌词和 Provider 故障隔离；
- BLAKE3、Chromaprint、五类重复组、质量版本对比与可选元数据 AI 建议；
- 安全 Hardlink Organizer、冲突保护、可恢复 `.meloark-trash` 与二次确认永久清理；
- Web 底部播放器、队列、同步歌词、收藏、历史、歌单与 FFmpeg 转码缓存；
- OpenSubsonic/Subsonic Server API，供 Symfonium 等客户端连接；
- 单用户管理员、持久化长任务、SSE 进度与审计日志。

首版发布同时支持 `linux/amd64` 与 `linux/arm64`、SQLite 与单 Docker 镜像。产品边界见 [`plan.md`](plan.md)，逐项审计见 [`docs/PLAN_AUDIT.md`](docs/PLAN_AUDIT.md)，实现证据见 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md)。

## Docker 启动

```bash
cp .env.example .env
# 修改 .env 中的 JWT Secret 与宿主机音乐目录
docker compose up -d --build
docker compose logs -f meloark
```

打开 `http://localhost:31000`，首次启动创建唯一管理员。宿主机目录必须先通过 Compose Volume 挂载；Web 中填写容器路径（例如 `/music/source`），不能填写 `/mnt/nas/...` 等宿主机路径。

只读来源和多曲库示例位于 [`examples/compose`](examples/compose)。完整部署与升级步骤见 [`docs/deployment.md`](docs/deployment.md)。

生产服务器应使用只拉取版本镜像的 [`compose.production.yaml`](compose.production.yaml)：

```bash
cp .env.production.example .env.production
# 创建三个宿主机目录，确保 UID/GID 10001 可写数据与整理目录
# 设置随机 JWT Secret，并固定 MELOARK_IMAGE 版本
docker compose --env-file .env.production -f compose.production.yaml up -d
```

## OpenSubsonic

客户端 Server URL 填 MeloArk 根地址，例如 `https://music.example.com`，使用管理员用户名和密码。支持 salt + token、JSON/XML、浏览、搜索、播放、封面、收藏、歌单、scrobble 与同步歌词扩展；详细兼容边界见 [`docs/opensubsonic.md`](docs/opensubsonic.md)。

## 本地开发

```bash
cd apps/server
cp config/config.local.yaml.example config/config.local.yaml
cargo run

# 另一个终端
cd apps/web
pnpm install --frozen-lockfile
pnpm dev
```

Vite 默认代理 `/api` 到 Rust 服务。工具链、配置覆盖和质量命令见 [`docs/development.md`](docs/development.md)。

## 安全基线

- 未经明确确认不删除源音乐；重复检测和 AI 只给建议。
- Tag、Organizer、Trash 与永久清理均采用独立 Preview → Confirm → Apply。
- Organizer 默认 Hardlink；跨文件系统不回退 Copy，冲突不覆盖。
- 文件路径 canonicalize 后必须仍属于配置的 Library Root。
- 管理员密码使用 Argon2；OpenSubsonic 凭据加密保存；日志不记录 query auth 或 Secret。

公开部署前阅读 [`docs/security.md`](docs/security.md)，并配置 HTTPS 与反向代理限流。备份恢复见 [`docs/backup-restore.md`](docs/backup-restore.md)。

## 文档

- [架构](docs/architecture.md)
- [Provider 边界](docs/providers.md)
- [部署与升级](docs/deployment.md)
- [备份与恢复](docs/backup-restore.md)
- [OpenSubsonic](docs/opensubsonic.md)
- [安全审查](docs/security.md)
- [开发环境](docs/development.md)

## License

MeloArk 采用 [Apache License 2.0](LICENSE)。容器中的 FFmpeg、Chromaprint 及应用依赖保留各自许可证，详见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
