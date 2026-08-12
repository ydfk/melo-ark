# MeloArk 首版计划审计

更新日期：2026-08-12

本文件把 `plan.md` 的 M0–M6 与显式 UI/数据要求映射到当前交付证据。详细实现说明见 `IMPLEMENTATION_STATUS.md`。

| 范围 | 当前结论 | 主要证据 |
| --- | --- | --- |
| M0 Bootstrap | 本地完成 | React/Axum monorepo、SQLite migration、首次管理员、SPA 静态托管、`linux/amd64` 镜像、健康检查、开发与部署文档 |
| M1 Library / Scan / Jobs | 本地完成 | 多曲库、Preflight、增量扫描、Tag/ffprobe、dev/inode Hardlink、Watch + Reconcile、持久化 Jobs/SSE、FTS、Dashboard、Table/Album Grid |
| M2 Tag / Organizer | 本地完成 | 单曲/批量编辑、清空、替换/正则、繁简、统一标点、文件名解析；Tag 生成文件名由 Organizer 模板安全实现；Preview/Confirm/Apply/Undo/Retry、快照、Hardlink、冲突、Trash/Restore |
| M3 Scraper / Lyrics | 本地完成 | QQ、NetEase、Kugou、MusicBrainz 稳定适配器；Kuwo/Migu 默认关闭 Beta 适配器；缓存/限流/超时/重试/熔断；候选评分与版本惩罚；封面候选；LRC 解析、评分、双语预览和安全写入 |
| M4 Duplicate / AI | 本地完成 | BLAKE3、Chromaprint、五类重复组、Quality Score、版本隔离、批量 Trash Preview、显式 AI 结构化元数据确认；独立 Hash/Fingerprint/Artwork 表 |
| M5 Player / OpenSubsonic | 本地完成 | Web 播放器、队列、歌词、Range、转码/缓存、历史/收藏/歌单；OpenSubsonic 认证、浏览、Search3、Stream、CoverArt、Star、歌单与歌词扩展；Symfonium 风格 JSON/XML 契约 |
| M6 UI / Release | 本地完成，外部发布待执行 | 响应式 UI、Command Palette、快捷键、Skeleton/空错态、50,001 曲目性能测试、安全审查、生产 Compose、amd64 CI/OCI 构建、健康的只读容器 |

## 显式 UI 清单

- Dashboard：统计、格式分布、缺失 Tag/歌词/封面、重复、最近扫描、运行任务、最近加入、最近播放。
- Library：服务端分页 Table、Album Art Grid、列开关、多选批处理、快捷筛选和完整计划列。
- Track Drawer：Overview、Tags、Files、Organizer、Scrape、Lyrics、Artwork、History。
- Duplicate Center：五类分析 Tab；成员显示封面、标题、版本、Codec、Bitrate、Sample Rate、Bit Depth、大小、Quality Score、Fingerprint Similarity 和路径。
- Tasks：进度、当前文件、成功/失败/跳过、速度、ETA、Pause、Resume、Cancel、Retry Failed。
- Settings：General、Libraries、Metadata Providers、Lyrics Providers、AI、Organizer、Playback、OpenSubsonic、Jobs、Storage、Security。

## 尚需外部条件

1. 创建并推送 `v1.2.3` 格式的版本 Tag，发布 Docker Hub `linux/amd64` 镜像并创建 GitHub Release。amd64 构建与生产 Compose 已完成，当前远端仍没有 Tag。

此项不影响本地代码、测试和容器交付，但在取得真实证据前不能写成“已完成”。
