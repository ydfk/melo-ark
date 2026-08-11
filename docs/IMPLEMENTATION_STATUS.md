# 实施状态

更新日期：2026-08-12

## M0 — Bootstrap

状态：完成。

- 已建立 `apps/web`、`apps/server`、`docs`、`fixtures/audio`。
- 已记录两个指定 starter 的精确 commit 基线并保留其质量脚本。
- 已替换 MeloArk 品牌、首页和占位 Logo。
- 已实现 `/api/health`、首次管理员状态/初始化、登录、当前用户。
- 已启用 SQLite WAL、foreign keys、busy timeout 与迁移。
- 已实现 Axum SPA 静态托管与 `linux/amd64`、`linux/arm64` 双架构单镜像 Dockerfile。
- 已提供 Compose、本地开发文档与配置环境变量覆盖。
- 后端 `fmt`、`clippy -D warnings`、`test`、`release build` 已通过。
- 前端 `format:check`、`lint`、`test:run`、`build` 已通过。
- Docker 相关门禁已在 M6 使用 OrbStack 统一补验。

## M1 — Library / Scan / Task Engine

状态：完成。

- 已实现多 Library Root、canonical path preflight、可读写能力检查与格式能力矩阵。
- Scanner 使用有界队列和可配置并发，逐项校验 Library Root 边界；外部 `ffprobe` 使用参数调用，不经过 shell。
- 已实现 Lofty Tag 读取、技术参数读取、path/size/mtime/dev/inode 增量判断，以及 hardlink 物理身份识别。
- Logical Track 与 Physical MediaFile 已分离；FTS5 可检索标题、歌手、专辑、标准化字段和物理路径。
- 中文搜索索引支持 NFKC、繁简归一、全拼和拼音首字母；升级时按索引版本一次性重建，`周杰倫`、`zhoujielun` 与 `zjl` 已由真实扫描集成测试覆盖。
- 已实现持久化 Job/Job Item、pause/resume/cancel/retry、重启中断恢复与 Bearer Header 鉴权 SSE。
- 同一曲库扫描严格串行；扫描运行或暂停期间的新请求会原子去重为一个后续任务，避免 Organizer Apply/Undo 的 reconciliation 请求被正在运行的旧扫描吞掉。
- 已实现 notify Watch 的代际重启与 debounce，并以周期 reconciliation 兜底 NAS Watch 不可靠场景。
- 前端已提供 Dashboard、Library Root 新增/预检/扫描、服务端分页 Table/Album Grid、列开关、快捷筛选、多选批处理和任务中心。
- 任务 API 与 UI 显示处理速度和 ETA；无采样数据返回 `null` 时会稳定显示“等待采样”，不会触发页面异常。
- 集成测试使用真实 WAV 与 hardlink：首次扫描识别相同 dev/inode，二次扫描全部走增量跳过。
- 已用真实浏览器验证首次初始化、登录、Dashboard、Library Root 预检新增、扫描和任务展示；浏览器控制台无 warning/error。
- 后端 `fmt`、`clippy -D warnings`、7 项测试与 `release build` 全部通过。
- 前端 `format:check`、`lint`、4 项测试与生产构建全部通过。
- Docker 单镜像与真实外部音频工具已在 M6 使用 OrbStack 统一补验。

## M2 — Tag / Batch / Organizer

状态：完成。

- 已实现 Track Drawer：概览、Tag、物理文件、Organizer、刮削、歌词、Artwork 与 History 八个工作区。
- 单曲与批量 Tag 支持仅设置填写字段、清空、trim、查找替换、正则替换、繁体转简体、统一中西文标点和 `03 - 标题` 文件名解析。
- 已支持 JPEG/PNG/WebP 嵌入封面（10 MiB 上限）；图片会经过 Lofty 格式校验。
- Tag 写入严格执行持久化 Preview Diff → Confirm → Apply；写入前保存完整嵌入元数据快照，支持 Undo 和失败项 Retry。
- Organizer 支持变量模板、跨平台文件名清理、缺失值 fallback、同文件系统预检和默认 Hardlink。
- 目标同 inode 视为幂等成功；不同 inode 明确标记 Path Conflict，Apply 不覆盖且不回退 Copy。
- Hardlink Apply 与 Undo 后都会触发目标曲库 reconciliation，索引不会保留已删除链接。
- 已实现每个 Library Root 下的 `.meloark-trash` Preview、显式确认、移动与 Restore。
- 永久清理是独立持久化 Preview → 精确二次确认 → Apply；Preview 和 Apply 都校验普通文件、回收站边界与 size/dev/inode，不递归删除。符号链接逃逸测试证明外部文件不会被删除。
- Tag、Organizer、Trash 的逐项结果同步到持久化统一 Jobs/Job Items，并通过 SSE 显示在 Task Center。
- Track History 合并文件/Tag 操作日志与 Web/OpenSubsonic 播放历史；操作失败原因和目标路径可直接追溯。
- OpenAPI 已覆盖 Track Detail、Tag、Organizer、Operation Journal 与 Trash 接口。
- 集成测试用真实音频验证 Tag/封面写入、简繁转换、索引重建、快照撤销、Hardlink、冲突不覆盖、Undo、Trash/Restore 和统一任务记录。
- 浏览器验证了真实音频的 Tag Diff/Apply、索引刷新、Organizer Dry Run/Apply/Undo 与任务中心；控制台无 warning/error。
- 后端 `fmt`、`clippy -D warnings`、13 项测试与 `release build` 全部通过。
- 前端 `format:check`、`lint`、6 项测试与生产构建全部通过，并将管理工作区按路由面板拆包。
- Docker 单镜像与真实音频扫描/播放链路已在 M6 使用 OrbStack 统一补验。

## M3 — Scraper / Lyrics

状态：完成。

- 已实现统一 `MetadataProvider` Trait、能力声明、结构化错误与独立适配器；Provider 不直接操作文件。
- Provider 的 endpoint、优先级、timeout 与 rate interval 可在 Web 配置；瞬时 HTTP/timeout 失败按配置有限退避重试，集成测试覆盖首次 503 后成功。
- QQ、NetEase、Kugou 与 MusicBrainz 支持元数据搜索；前三者的网页接口由缓存、限流、超时、熔断和契约 fixture 隔离，接口变化只会形成单源故障。
- Kuwo、Migu 已实现独立的可配置 Beta 适配器和响应解析测试，默认关闭；LrcApi-compatible endpoint 同样默认关闭，不会影响稳定 Provider。
- MusicBrainz 使用官方 `/ws/2/recording` JSON 搜索，并按官方要求设置有意义的 User-Agent 和 1 req/s 限流。
- 候选评分按 Title/Artist/Album/Duration/Track/Year 加权；Live、Remix、Remaster、Instrumental、翻唱/伴奏等版本不一致会受到强惩罚。
- 95 分以上仅进入高置信候选；80–94 分必须提交 `APPLY_REVIEWED`；低分候选需要额外风险确认。所有应用仍先生成 Tag Diff。
- 已实现 Provider 舱、Track Drawer 刮削工作台、封面候选、差异高亮与持久化批量刮削 Job。
- 歌词支持本地外置 `.lrc`、内嵌 Lyrics、QQ/NetEase/Kugou 候选、同步时间轴解析、覆盖率/质量评分与双语同时间戳预览。
- 歌词写入支持仅外置、仅内嵌和两者；已有歌词默认返回冲突，只有显式 `replaceExisting` 才能替换。
- 歌词写入创建持久化 Job；逐项失败可在任务中心重试，集成测试覆盖先因已有 LRC 失败、移除冲突后重试成功。
- 真实 WAV 集成测试覆盖 80–94 分禁止普通确认、候选 Diff、批量任务持久化、本地 LRC 评分及禁止静默覆盖。
- 后端 `fmt`、`clippy -D warnings`、测试与 `release build` 已通过；前端 `format`、`lint`、测试和生产构建已通过。
- Docker、FFmpeg 与 fpcalc 外部二进制已在 M6 镜像内统一补验。

## M4 — Duplicate / Fingerprint / AI

状态：完成。

- 分析任务按物理文件去重后执行 BLAKE3 与 Chromaprint；支持暂停、取消、失败重试和持久化逐项结果。
- Hash、Fingerprint 与封面元信息分别落入 `audio_hashes`、`audio_fingerprints`、`artworks`；旧 `media_files` 分析列继续兼容已有查询和升级数据。
- 重复组分离为 hardlink alias、binary exact、audio duplicate、quality variant 与 possible duplicate；Hardlink 不计可回收空间。
- Quality Score 综合 codec、bit depth、sample rate、bitrate 和声道；Live、Remix、Remaster、Instrumental 等版本不会被错误折叠。
- 重复工作台支持类型筛选、成员选择、推荐保留项、可回收空间和 Trash Preview；分析本身不会删除文件。
- AI 默认关闭，只允许显式确认后上传结构化元数据，不上传音频；支持候选 rerank 与重复原因解释。
- 集成测试覆盖 copy、hardlink、不同码率/质量、Live、Remix、Remaster、Instrumental，以及 fpcalc“非零退出但返回有效指纹”的 Debian 运行时边界。
- 后端格式、Clippy、集成测试与 release build 均已通过。

## M5 — Player / FFmpeg / OpenSubsonic

状态：完成。

- Web 已实现底部播放器、播放队列、随机/循环、进度与音量、收藏、历史、歌单和同步歌词。
- 原始播放支持 Bearer/短期 scoped token 与标准 HTTP Range；转码提供 Opus 192、AAC 256、MP3 320 三个 profile，并按输入/参数缓存和 LRU 限额清理。
- OpenSubsonic 支持 salt + token、JSON/XML、Music Folders、Indexes、Artists、Albums、Random、Search3、Song、Directory、Stream/Download、CoverArt、Star、Scrobble、Now Playing、Playlist CRUD 与结构化歌词扩展。
- Search3 与 Web 共用中文标准化 FTS，支持全拼/拼音首字母；中文艺术家索引使用拼音首字母，真实容器已验证 `zjl` 命中周杰倫。
- 自动化的 Symfonium 风格契约覆盖 XML ping、JSON 浏览/搜索、Range 播放、嵌入封面、收藏、歌单和歌词扩展；错误凭据返回 Subsonic code 40。
- OrbStack 容器内使用真实 FFmpeg 5.1.9 完成 FLAC → Opus 转码，返回 `audio/ogg`；原始 Web 与 OpenSubsonic Range 均返回 206。
- 首版以 OpenSubsonic 服务端兼容契约作为发布门禁；Symfonium 等第三方客户端的实体设备 UI 属于后续可选验收，不阻塞当前发布。

## M6 — UI Polish / Release

状态：代码与本地发布门禁完成；尚未向外部 GitHub/GHCR 发布。

- Dashboard 延续唱片纹理与冷蓝控制台方向；桌面和 390×844 移动端均完成登录、总览、曲库双视图、任务、Provider、播放器、回收站、设置与横向导航视觉验收。
- Dashboard 显示曲目/艺术家/专辑/物理文件、缺失 Tag/歌词/封面、精确/可能重复、格式分布、最近扫描、最近加入和最近播放；Library Root 支持编辑、预检和仅删除配置/索引。
- 已实现完整 Settings 页签、Command Palette、`⌘/Ctrl+K`、`Ctrl+1…8`、Skeleton、加载失败重试、空状态、Provider 独立故障说明和回收站管理入口。
- 50,001 曲目真实 API 性能测试通过：本轮分页列表约 328 ms、FTS 搜索约 228 ms，均低于 5 秒门禁。
- 登录按用户名限制一分钟内 5 次失败；路径操作限制在 canonicalized Library Root；外部命令使用参数列表且不经过 shell；敏感 query 不进入请求日志字段。
- 已提供 Apache-2.0 `LICENSE`、第三方声明、部署、备份恢复、OpenSubsonic、安全文档、只读/多曲库 Compose 示例。
- CI 覆盖 Rust fmt/clippy/test/release、Web format/lint/test/build 与 `linux/amd64`、`linux/arm64` 镜像构建；Tag workflow 可向 GHCR 发布带 provenance 与 SBOM 的多架构清单，并创建附带开发/生产 Compose、环境模板、许可证和第三方声明的 GitHub Release。
- OrbStack 已分别运行当前源码的原生 `linux/arm64` 与模拟 `linux/amd64` 生产容器，验证只读根文件系统、`no-new-privileges`、UID/GID 10001 与健康检查；真实扫描、FFmpeg、fpcalc、Range、转码及 OpenSubsonic 链路也已通过容器验收。
- GitHub/Gitea 推送目标已配置，双架构与生产 Compose 已进入远端 `main`；远端仍没有发布 Tag，因此 GHCR 多架构镜像与 GitHub Release 尚未创建。本轮扫描竞态修复尚未提交或推送。

## 最终质量门禁

- Server：47 项测试通过，`cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --all-targets --all-features`、`cargo build --release --locked` 通过。
- Web：7 项测试通过，`pnpm format:check`、`pnpm lint`、`pnpm test:run`、`pnpm build` 通过。
- Container：开发/生产 Compose 配置通过；当前源码经 Buildx 实际导出的 OCI 索引同时包含 `linux/amd64` 与 `linux/arm64`。生产容器已在原生 arm64 与模拟 x86_64 环境分别验证健康检查、UID/GID 10001、只读根目录与禁止提权。
- Browser：隔离容器中完成首次初始化和 3 首真实 FLAC 扫描；桌面与 390×844 移动端控制台无 warning/error，FLAC 可播放。

## 外部完成边界

- 远端仍没有版本 Tag；在 Tag workflow 成功发布 GHCR 多架构清单与 GitHub Release 前，不能把外部发布写成已完成。
