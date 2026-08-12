<!-- markdownlint-disable MD033 MD041 -->

<div align="center">
  <img src="apps/web/public/meloark-mark.svg" alt="MeloArk" width="96" height="96">
  <h1>MeloArk</h1>
  <p>为 NAS 与 HomeLab 打造的自托管音乐管理、整理与播放服务</p>

  <p>
    <a href="https://github.com/ydfk/melo-ark/releases">
      <img
        src="https://img.shields.io/github/v/release/ydfk/melo-ark?display_name=tag&style=flat-square"
        alt="GitHub Release"
      >
    </a>
    <a href="https://hub.docker.com/r/ydfk/meloark">
      <img
        src="https://img.shields.io/docker/pulls/ydfk/meloark?style=flat-square"
        alt="Docker Pulls"
      >
    </a>
    <img
      src="https://img.shields.io/badge/platform-linux%2Famd64-2563eb?style=flat-square"
      alt="Platform linux/amd64"
    >
    <a href="LICENSE">
      <img
        src="https://img.shields.io/github/license/ydfk/melo-ark?style=flat-square"
        alt="License"
      >
    </a>
  </p>

  <p>
    <a href="#快速开始">快速开始</a> ·
    <a href="#核心能力">核心能力</a> ·
    <a href="#生产部署">生产部署</a> ·
    <a href="#opensubsonic">OpenSubsonic</a> ·
    <a href="#开发">开发</a> ·
    <a href="#文档">文档</a>
  </p>
</div>

MeloArk 索引已经挂载到服务器的本地音乐文件，在一个 Web 工作台中完成扫描、
搜索、Tag 修订、元数据补全、重复分析、安全整理和播放。服务采用 React + Axum +
SQLite 构建，并在单个 Docker 镜像中内置 FFmpeg、ffprobe 与 Chromaprint。

> [!IMPORTANT]
> MeloArk 不提供在线音乐下载。当前发布仅支持 `linux/amd64`；Windows Docker
> Desktop 需要使用 Linux containers 模式，暂不支持 ARM64 与 Windows Containers。

## 核心能力

| 场景 | 能力 |
| --- | --- |
| 曲库管理 | 多曲库增量扫描、NAS 文件监听与周期校准、持久化任务、SSE 实时进度 |
| 搜索与浏览 | SQLite FTS5、中英文归一、全拼与拼音首字母搜索、分页表格与专辑视图 |
| 元数据 | 单曲/批量 Tag、封面、同步歌词，以及 QQ、网易、酷狗、MusicBrainz 候选 |
| 安全整理 | 所有写入先 Preview，再 Confirm 与 Apply；支持快照撤销、失败重试和操作审计 |
| 重复分析 | BLAKE3、Chromaprint、Hardlink 识别、质量版本对比与可选 AI 建议 |
| 播放体验 | Web 播放器、队列、收藏、歌单、历史、同步歌词与 FFmpeg 转码缓存 |
| 客户端兼容 | OpenSubsonic/Subsonic API，可供 Symfonium 等兼容客户端连接 |

Organizer 默认使用 Hardlink，不覆盖冲突文件，也不会在跨文件系统失败时自动复制。删除流程先进入每个曲库内的 `.meloark-trash`，永久清理需要单独确认。

## 快速开始

需要 Docker Engine 及 Docker Compose，并运行在 `linux/amd64` 环境。

```bash
git clone https://github.com/ydfk/melo-ark.git
cd melo-ark
cp .env.example .env
```

编辑 `.env`：

```dotenv
MELOARK_JWT_SECRET=替换为至少32位的随机字符串
MELOARK_SOURCE_PATH=/实际的音乐目录
MELOARK_MANAGED_PATH=/实际的整理目录
```

从源码构建并启动：

```bash
docker compose up -d --build
docker compose logs -f meloark
```

打开 <http://localhost:31000>。空数据库首次启动时会自动创建 `admin/admin`，凭据同时打印到服务日志；首次登录后必须修改默认密码。随后在曲库页面通过目录树选择音乐目录。

> [!NOTE]
> Web 中填写的是容器路径，不是 `/mnt/nas/music` 等宿主机路径。宿主机目录与容器路径的对应关系由 Compose Volume 决定。

## 生产部署

生产环境应固定版本镜像，不在服务器上现场构建：

```bash
cp .env.production.example .env.production
# 生成 Secret，例如：openssl rand -hex 32
# 设置 MELOARK_IMAGE、MELOARK_JWT_SECRET 和三个宿主机目录

docker compose --env-file .env.production -f compose.production.yaml config --quiet
docker compose --env-file .env.production -f compose.production.yaml pull
docker compose --env-file .env.production -f compose.production.yaml up -d
```

| 配置 | 容器路径 | 要求 |
| --- | --- | --- |
| `MELOARK_DATA_PATH` | `/data` | 本地持久化目录，UID/GID `10001` 可写 |
| `MELOARK_SOURCE_PATH` | `/music/source` | 音乐来源；修改 Tag 时需要可写 |
| `MELOARK_MANAGED_PATH` | `/music/organized` | Organizer 目标目录，需要可写 |

生产容器以非 root 用户运行，并启用只读根文件系统、禁止提权和健康检查。SQLite 数据应保存在本地 `/data`，不要放入 SMB/NFS 音乐目录。

- [只读音乐来源示例](examples/compose/compose.read-only-source.yaml)
- [多曲库挂载示例](examples/compose/compose.multiple-roots.yaml)
- [完整部署、升级与离线镜像说明](docs/deployment.md)

## OpenSubsonic

兼容客户端使用以下连接信息：

| 字段 | 内容 |
| --- | --- |
| Server URL | MeloArk 根地址，例如 `https://music.example.com` |
| Username / Password | MeloArk 管理员用户名和密码 |
| API version | `1.16.1` |
| 推荐认证 | salt + token |

已支持浏览、搜索、播放、下载、封面、收藏、Scrobble、Now Playing、歌单和结构化歌词。
对外开放时应通过 HTTPS 反向代理访问，完整兼容范围见
[OpenSubsonic 文档](docs/opensubsonic.md)。

## 镜像与发布

推送 `v1.2.3` 或 `v1.2.3-rc.1` 格式的 Git Tag 会自动构建并发布
`linux/amd64` 镜像。维护者也可以在 GitHub Actions 中手动运行
**Build Docker image and Release**，并填写一个已经存在的 Tag。

稳定版本会生成完整版本、次版本、主版本和 `latest` 标签；预发布版本不会更新
`latest`。同一流程会创建或更新 GitHub Release，并附带 Compose、环境模板和第三方声明。

## 开发

工具链版本：Node.js 26.5.0、pnpm 11.17.0、Rust 1.97.1。

启动 Server：

```bash
cd apps/server
cp config/config.local.yaml.example config/config.local.yaml
cargo run
```

启动 Web：

```bash
cd apps/web
pnpm install --frozen-lockfile
pnpm dev
```

Vite 默认将 `/api` 代理到 Rust 服务。格式化、Lint、测试和生产构建命令见
[开发文档](docs/development.md)。

## 架构

MeloArk 首版采用模块化单体架构：Axum 提供 API、OpenSubsonic 接口和 React 静态资源，
SQLite 保存业务数据与持久化任务，媒体工具由容器统一提供。Logical Track 与 Physical
MediaFile 分离，因此同一首歌可以安全关联多个编码或质量版本。

详细数据流与安全边界见 [架构说明](docs/architecture.md)。

## 文档

- [部署与升级](docs/deployment.md)
- [备份与恢复](docs/backup-restore.md)
- [安全基线](docs/security.md)
- [在线数据源运行与合规边界](docs/providers.md)
- [OpenSubsonic 兼容说明](docs/opensubsonic.md)
- [开发环境](docs/development.md)
- [实现状态与验证证据](docs/IMPLEMENTATION_STATUS.md)
- [产品计划](plan.md)
