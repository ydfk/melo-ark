# MeloArk 开发计划

> 版本：v0.1 Draft
> 日期：2026-08-09
> 定位：面向 NAS / HomeLab / 本地音乐收藏的自托管 Web 音乐管理、整理与播放服务。
> 首版平台：Linux `amd64` / `arm64`，单 Docker 镜像，SQLite。

---

## 1. 项目名称与品牌

### 推荐名称：MeloArk

- `Melo`：Melody / Music。
- `Ark`：方舟、归档容器，表达“把散乱音乐安全收纳、识别、整理成自己的曲库”。
- 仓库名建议：`meloark`
- Docker 镜像建议：`ghcr.io/<owner>/meloark`
- Web 标题：`MeloArk`
- 中文描述：`MeloArk - 自托管本地音乐管理与整理中心`

### 备选名称

1. **MeloArk** — 推荐，覆盖“音乐 + 收藏/归档”整体定位。
2. **Riffarium** — 偏音乐收藏馆概念，但略偏乐队/摇滚语义。
3. **SonaDock** — 声音停靠站，适合 NAS / HomeLab。
4. **Trackory** — Track + Repository/Inventory，偏管理工具。
5. **MetaTune** — 强调元数据，但不足以表达播放、Subsonic 和曲库能力。

> 正式公开发布前仍应做一次 GitHub、容器仓库、域名和商标重名检查。

### Logo 方向

统一视觉符号由以下元素抽象组合：

- 黑胶唱片 / 音频圆盘；
- 方舟 / 收藏盒 / 归档容器；
- 标签 Tag 的切角；
- 中心可隐约形成字母 `M` 或波形；
- 图标必须在 16px favicon 下仍能识别；
- 深色 UI 中使用蓝紫青渐变作为品牌高光；
- 同时保留可做纯黑/纯白单色版本的几何结构。

需要两套正式资产：

- `Icon`：纯图标，用于 favicon、GitHub Avatar、Docker、PWA。
- `Wordmark`：图标 + `MeloArk` 英文字标，用于登录页、README、Web Header。

---

## 2. 已确定的产品约束

### 2.1 基本约束

- 主要管理本地/NAS音乐，不提供在线音乐下载。
- 当前曲库规模约 `3TB`。
- 支持多个音乐根目录。
- 首版同时支持 Linux `amd64` 与 `arm64`。
- 单用户管理员模式。
- 数据库仅支持 SQLite。
- 单 Docker 镜像交付。
- 前端基于：
  - `https://github.com/ydfk/react-starter`
- 后端基于：
  - `https://github.com/ydfk/rust-axum-starter`

### 2.2 核心功能

- 曲库扫描与索引
- Tag 查看、单曲编辑、批量编辑
- 元数据刮削
- 中文歌词获取、比较、写入
- 封面处理
- 四种维度的重复检测
- 音频指纹
- 文件整理
- 默认硬链接
- Dry Run
- 操作历史与回收站
- Web 简易播放
- FFmpeg 转码
- OpenSubsonic / Subsonic Server API
- 长任务中心
- 可选 AI 辅助
- 中文环境优先
- Web UI 深色、现代、具有音乐播放器视觉效果

### 2.3 安全原则

任何功能都必须遵守：

1. **未经用户明确执行，不删除音乐文件。**
2. **重复检测只给结论和建议，不自动删。**
3. **整理默认硬链接，不允许跨文件系统时自动降级复制。**
4. **批量 Tag / 文件整理先 Preview，再 Apply。**
5. **AI 只能提供建议，不能直接执行破坏性操作。**
6. **源文件操作必须保留审计记录。**
7. **Provider 失败不能影响本地曲库核心功能。**

---

# 3. 总体技术架构

## 3.1 代码结构

建议新项目采用一个仓库：

```text
meloark/
├── apps/
│   ├── web/                    # 基于 ydfk/react-starter
│   └── server/                 # 基于 ydfk/rust-axum-starter
├── docs/
│   ├── architecture.md
│   ├── provider.md
│   ├── opensubsonic.md
│   └── development.md
├── docker/
│   └── ...
├── fixtures/
│   └── audio/                  # 测试音频，小文件
├── compose.yaml
├── Dockerfile
├── LICENSE
├── THIRD_PARTY_NOTICES.md
├── README.md
└── plan.md
```

不要求强行把两个 starter 的历史合并，只需要保留它们现有的工程约定、依赖版本、质量检查方式和 API 风格。

## 3.2 前端

沿用 `react-starter`：

- React
- TypeScript
- Vite
- Tailwind CSS
- shadcn/ui / Radix UI
- Zustand
- Alova
- React Hook Form
- Zod
- TanStack Table
- Vitest

允许新增：

- `@tanstack/react-virtual`：超大曲目列表虚拟滚动。
- 轻量 Motion 库：用于页面和播放器微动效，禁止为了视觉效果引入重型 3D。
- 专门的 LRC 解析工具可自行实现，避免引入不必要依赖。

默认简体中文，保留 i18n 扩展能力，但第一版不要求完整多语言。

## 3.3 后端

沿用 `rust-axum-starter`：

- Rust 2024 Edition
- Axum
- Tokio
- SQLx
- SQLite
- Utoipa / OpenAPI
- JWT
- Argon2
- tracing
- `unsafe_code = forbid`

新增核心依赖优先考虑：

- `lofty`：常见音频 Tag 读写。
- `blake3`：完整文件 Hash。
- `notify`：文件监听。
- `reqwest`：Provider HTTP。
- `async-trait`：Provider 抽象。
- `tokio-util`：CancellationToken。
- `walkdir` 或异步等价方案：目录扫描。
- `unicode-normalization`：字符串标准化。
- 简繁转换：采用 OpenCC 兼容 Rust 实现，实际编码前验证维护状态。
- 拼音：选择维护活跃的 Rust 拼音库，用于首字母整理和辅助搜索。

外部 Runtime 工具：

- FFmpeg / ffprobe
- Chromaprint / `fpcalc`

### 为什么 Tag 和播放不完全依赖同一个 Rust 音频库

MeloArk 面向 NAS 中实际存在的复杂格式。

Tag 层优先由 Lofty 处理其支持的格式；播放/技术信息/转码优先依赖 FFmpeg。这样避免因为纯 Rust 解码器暂不支持 APE、DSD、WMA 等格式而阻塞整个产品。

首版明确区分：

- `metadata_read`
- `metadata_write`
- `direct_browser_play`
- `ffmpeg_transcode`
- `fingerprint`

每个格式分别声明能力，不做“看到扩展名就认为所有能力都支持”的假设。

---

# 4. Docker 与文件系统模型

## 4.1 单镜像

最终生产镜像包含：

- Rust Server
- React 构建产物
- FFmpeg / ffprobe
- Chromaprint / fpcalc
- CA Certificates

React 静态文件由 Axum 直接托管。

SPA 路由需要 fallback 到 `index.html`。

首版构建同一镜像标签下的两个平台：

```text
linux/amd64
linux/arm64
```

## 4.2 挂载模型

Docker 无法由 Web 页面动态获得新的宿主机目录，因此：

用户先通过 Compose 挂载：

```yaml
volumes:
  - ./data:/data
  - /mnt/nas/music:/music/source:rw
  - /mnt/nas/music-organized:/music/organized:rw
```

Web 中配置的是容器内部路径：

```text
/music/source
/music/organized
```

UI 必须明确提示这一点。

## 4.3 Library Root

允许多个 Root：

```text
LibraryRoot
- id
- name
- path
- scan_enabled
- watch_enabled
- writable
- role
- exclude_patterns
```

`role`：

- `source`
- `managed`
- `both`

支持排除目录，例如：

```text
@eaDir
.recycle
.meloark-trash
lost+found
```

## 4.4 SQLite 放置要求

SQLite 数据库必须位于 `/data` 等本地持久化目录。

**不要建议用户把 SQLite DB 放在 NFS/SMB 音乐目录。**

默认启用：

- WAL
- busy timeout
- foreign keys
- 合理的 SQLite connection pool
- FTS5 搜索索引

---

# 5. 数据模型

设计原则：

> **逻辑歌曲 Track 和物理音频文件 MediaFile 分离。**

原因是同一首歌可以同时存在：

```text
MP3 320k
FLAC 16/44.1
FLAC 24/96
WAV
不同发行版
不同 Remaster
```

并且用户明确要求允许保留多个质量版本。

## 5.1 核心表

### libraries

音乐根目录。

### artists

```text
id
name
sort_name
normalized_name
```

### albums

```text
id
title
album_artist
year
cover_art_id
```

### tracks

逻辑歌曲：

```text
id
title
normalized_title
album_id
track_no
disc_no
year
genre
duration_ms
version_label
created_at
updated_at
```

### track_artists

支持多 Artist。

### media_files

物理文件：

```text
id
track_id
library_id
relative_path
extension
file_size
mtime
device_id
inode
hardlink_count

codec
container
duration_ms
bitrate
sample_rate
bit_depth
channels

metadata_readable
metadata_writable
fingerprint_status
hash_status

created_at
updated_at
```

### embedded_metadata_snapshots

保存扫描时和变更前的 Tag 快照，用于 Diff / Audit / Undo。

### audio_hashes

```text
media_file_id
blake3
calculated_at
source_size
source_mtime
```

### audio_fingerprints

```text
media_file_id
algorithm
fingerprint
duration_ms
calculated_at
```

### lyrics

```text
id
track_id
kind
language
source
synced
translated
content
quality_score
external_path
embedded_state
```

### artworks

封面元信息；图片本体优先放 `/data/cache/artwork`，SQLite 不存大 Blob。

### scrape_candidates

保存 Provider 候选及评分。

### duplicate_groups
### duplicate_members

保存重复分析结果，而不是直接修改文件。

### jobs
### job_items

持久化任务和任务条目。

### operations
### operation_items

记录用户实际执行的文件/Tag 修改。

### playlists
### playlist_items
### play_history
### favorites

为 Web Player 和 OpenSubsonic 使用。

### provider_cache

缓存外部 Provider 响应，降低风控与重复请求。

---

# 6. 曲库扫描

## 6.1 扫描模式

全部支持：

1. 首次全量扫描
2. 手动扫描
3. 定时扫描
4. 文件系统 Watch

注意：

> NAS / NFS / SMB 上文件 Watch 不一定可靠，因此 Watch 只能作为加速机制，周期性 Reconcile 才是最终一致性的保障。

## 6.2 扫描阶段

### Phase A：文件枚举

只扫描支持的扩展名。

读取：

- path
- size
- mtime
- inode
- device id
- hardlink count

不立即读取整文件。

### Phase B：Tag 与技术信息

读取：

- Title
- Artist
- Album
- AlbumArtist
- Track
- Disc
- Year
- Genre
- Cover
- Duration
- Codec
- Bitrate
- Sample rate
- Bit depth
- Channels

优先使用 Rust 库，必要时通过 ffprobe 补充。

### Phase C：增量判断

若：

```text
path + size + mtime + inode
```

未变化，则不重复解析 Tag。

### Phase D：Hash / Fingerprint

不在基础扫描中强制读取全部 3TB。

Hash 和 Fingerprint：

- 用户主动执行；
- 后台低优先级任务；
- 或只对重复候选计算。

这样避免每次扫描都顺序读取 TB 级音频。

## 6.3 Hardlink 识别

如果两个路径：

```text
device_id 相同
inode 相同
```

则标记：

```text
hardlink_alias
```

不能把它们当作两个真实占用空间的重复文件。

---

# 7. Tag 管理

## 7.1 首版常规字段

第一阶段：

- Title
- Artist
- Album
- AlbumArtist
- Track Number
- Disc Number
- Year
- Genre
- Cover
- Lyrics

高级字段后续再做。

## 7.2 编辑模式

支持：

- 单曲编辑
- 多选批量编辑
- 仅修改填写字段
- 清空指定字段
- 查找/替换
- 正则替换
- 繁体转简体
- 去除首尾空格
- 统一标点
- 从文件名解析 Tag
- 根据 Tag 生成文件名
- 批量 AlbumArtist

## 7.3 两阶段写入

任何批量 Tag 修改：

```text
Draft
  ↓
Preview Diff
  ↓
Confirm
  ↓
Apply
  ↓
Verify
  ↓
Operation Log
```

Diff 示例：

```text
Artist
周杰倫
→
周杰伦
```

写入前保存 Tag Snapshot。

失败不能导致整个批任务全部中止，应记录失败项并允许 Retry。

---

# 8. 中文元数据标准化

中文曲库是一级设计目标，不是英文逻辑加翻译。

## 8.1 内部匹配标准化

仅用于搜索/匹配，不改变用户原始 Tag：

- Unicode NFKC
- 全角转半角
- 大小写折叠
- 简繁标准化
- 常见中文/英文标点归一
- 多余空格清理
- `feat.` / `ft.` / `featuring` 规范化
- Artist 分隔符标准化
- 中文括号与英文括号标准化

## 8.2 版本关键词

必须识别并纳入评分：

```text
Live
现场
演唱会
Remix
Mix
DJ
伴奏
Instrumental
Acoustic
Unplugged
Remaster
Remastered
重制
Demo
Radio Edit
Single Version
Album Version
Edit
Cover
翻唱
```

版本关键词不同必须施加强惩罚，避免：

```text
原版
Live
Remix
伴奏
Remaster
```

被错误归为同一个音频重复。

## 8.3 拼音

拼音仅用于：

- 中文歌手首字母分类；
- 辅助搜索；
- 整理目录模板 `{artist_initial}`。

不要自动把中文 Tag 替换成拼音。

---

# 9. 元数据刮削架构

## 9.1 Provider 抽象

定义统一 Trait：

```rust
trait MetadataProvider {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;

    async fn search_track(
        &self,
        query: TrackQuery
    ) -> Result<Vec<TrackCandidate>>;

    async fn get_track(
        &self,
        id: &str
    ) -> Result<TrackMetadata>;

    async fn get_cover(
        &self,
        id: &str
    ) -> Result<Vec<ArtworkCandidate>>;
}
```

歌词独立：

```rust
trait LyricsProvider {
    async fn search_lyrics(
        &self,
        query: LyricsQuery
    ) -> Result<Vec<LyricsCandidate>>;
}
```

Provider 不得直接操作文件。

## 9.2 Provider 优先级

默认推荐：

1. QQ Music
2. NetEase Cloud Music
3. Kugou
4. Kuwo
5. Migu
6. MusicBrainz

MusicBrainz 作为开放元数据兜底。

Apple Music 可以以后作为需要用户 Token 的可选 Provider，不作为首版硬依赖。

## 9.3 Provider 设计要求

每个 Provider 独立模块：

```text
providers/
├── qq/
├── netease/
├── kugou/
├── kuwo/
├── migu/
└── musicbrainz/
```

统一具备：

- timeout
- retry
- rate limit
- response cache
- User-Agent
- circuit breaker
- health status
- enable/disable
- priority
- mock fixtures

任何单个 Provider 故障不能导致刮削系统不可用。

## 9.4 匹配评分

基础权重建议：

```text
Title       40%
Artist      30%
Album       12%
Duration    10%
Track No     4%
Year         4%
```

额外：

- Version Label mismatch：强惩罚。
- Duration 差异超过阈值：强惩罚。
- 完全一致 ISRC 等高级字段以后可直接加分。

阈值：

```text
>= 95     高置信度，可进入自动接受队列
80 - 94   必须人工确认
< 80      不自动应用
```

即便达到 95，默认也只代表“可自动接受”，具体是否启用自动写入由设置决定。

## 9.5 多源结果 UI

候选界面显示：

```text
Provider
Score
Title
Artist
Album
Duration
Year
Track
Cover
版本信息
差异高亮
```

允许用户：

- 选一个 Provider 整体结果；
- 或分别选择 Metadata / Cover / Lyrics 来源。

---

# 10. 歌词系统

## 10.1 来源

优先内部实现 Provider，不依赖单独部署 LrcApi。

参考 LrcApi 的聚合思路，但重新实现，不复制 GPL 源代码。

歌词 Provider 优先：

1. Kugou
2. QQ Music
3. NetEase
4. 其他可维护来源
5. 可选外部 LrcApi-compatible Provider

## 10.2 首版格式

支持：

- 外置 `.lrc`
- 内嵌歌词
- 同步 LRC
- 双语歌词

逐字歌词的数据模型预留，但首版不要求所有 Provider 都能提供。

## 10.3 Lyrics Candidate

每个候选计算质量分：

```text
同步歌词            +++
时间轴覆盖完整       +++
时长吻合             +++
行数合理             ++
来源优先级           ++
包含翻译             可配置加分
乱码/重复行/空歌词    ---
时间轴严重越界       ---
```

UI 显示：

```text
质量评分 92
同步
双语
来源：Kugou
覆盖率：98%
```

用户自己选择。

## 10.4 覆盖策略

默认：

> 永远不静默覆盖已有歌词。

操作时提供：

- 保留本地
- 使用候选
- 仅写外置 LRC
- 仅写内嵌
- 两者都写

## 10.5 外置歌词

默认：

```text
歌曲.flac
歌曲.lrc
```

UTF-8。

双语写入需要保留相同 timestamp 下的原文与译文。

---

# 11. 重复检测

这是 MeloArk 的核心卖点之一。

用户可选择“本次按哪种维度分析”，不能把所有重复概念混成一个按钮。

## 11.1 Level 1：物理 Hardlink Alias

条件：

```text
device + inode 相同
```

结论：

```text
同一个物理文件的多个路径
```

不是浪费磁盘的真实重复。

## 11.2 Level 2：完全文件重复

计算：

```text
BLAKE3(full file)
```

相同则：

```text
Binary Exact Duplicate
```

即字节完全相同。

## 11.3 Level 3：音频内容重复

使用 Chromaprint / fpcalc。

目标：

> 即使 FLAC 与 MP3 编码不同，也判断是否来自同一段音频内容。

需结合：

- fingerprint similarity
- duration tolerance
- metadata

不得只比较文件大小。

## 11.4 Level 4：同一歌曲的不同质量版本

例如：

```text
周杰伦 - 晴天.mp3
周杰伦 - 晴天.flac
周杰伦 - 晴天 24-96.flac
```

可归入：

```text
Same Recording / Quality Variants
```

但全部允许保留。

## 11.5 Level 5：疑似重复

使用：

- normalized title
- normalized artist
- duration
- album
- fingerprint
- version tokens

生成：

```text
Possible Duplicate
```

必须人工判断。

## 11.6 Quality Score

Quality Score 只代表：

> 文件技术规格的比较分，不宣称代表真实听感。

参考：

- lossless / lossy
- codec
- bitrate
- bit depth
- sample rate
- channels
- 是否可正常解码
- 是否损坏

例如：

```text
FLAC 24/96  -> 92
FLAC 16/44  -> 82
MP3 320     -> 66
MP3 128     -> 42
```

但：

- 不因为 WAV 就一定优于 FLAC；
- 不因为 192kHz 就直接判断是真 Hi-Res；
- 不自动删除较低分版本。

## 11.7 AI 辅助

AI 仅用于难判断场景：

```text
晴天
晴天 (Live)
晴天 2025 Remaster
晴天 (DJ版)
晴天 (伴奏)
```

传递给 AI：

- 文件名
- Tag
- 技术参数
- 时长
- Provider 候选
- 版本关键词

默认不上传原始音频。

AI 输出：

```json
{
  "relation": "different_version",
  "confidence": 0.93,
  "reason": "one track is marked Live and duration differs by 37 seconds"
}
```

AI 结果仅作为 Recommendation。

---

# 12. AI 接口

AI 不作为核心依赖。

配置采用 OpenAI-compatible 形式：

```text
enabled
base_url
api_key
model
timeout
```

支持：

- OpenAI-compatible 云端接口
- 用户自己的本地模型网关

首版 AI 场景：

1. 疑似重复版本判断
2. 多 Provider 刮削候选 rerank
3. 混乱文件名 / Tag 的结构化建议

AI 禁止：

- 自动删除文件
- 绕过 Preview
- 未经用户设置自动发送整份曲库数据
- 上传音频文件

---

# 13. 音乐整理

## 13.1 默认方式

默认：

```text
Hardlink
```

另外架构预留：

- Copy
- Move
- Rename

但默认 UI 强调 Hardlink。

## 13.2 Hardlink 约束

执行前检查源与目标父目录：

```text
st_dev
```

不同：

```text
直接失败
```

禁止自动 fallback 为 Copy。

应用时仍必须处理真实 `hard_link()` 返回错误。

## 13.3 幂等

如果目标已经存在：

### 同 inode

视为：

```text
Already Organized
```

成功且无需重复创建。

### 不同 inode

标记：

```text
Path Conflict
```

默认不覆盖。

## 13.4 路径模板

默认：

```text
{artist}/{album}/{track:02} - {title}.{ext}
```

可自定义：

```text
{artist_initial}/{artist}/{year} - {album}/{disc:02}-{track:02} {title}.{ext}
```

变量首版：

```text
artist
artist_initial
album_artist
album
title
track
disc
year
genre
ext
quality
```

缺失字段必须定义 fallback。

## 13.5 文件名清理

处理：

- `/`
- NUL
- 连续空格
- 不合理路径长度
- `.` / `..`
- 控制字符

可提供“跨平台安全文件名”开关。

## 13.6 Dry Run

整理必须先生成：

```text
Operation Plan
```

示例：

```text
Source:
 /music/source/a.flac

Target:
 /music/organized/周杰伦/叶惠美/03 - 晴天.flac

Method:
 hardlink

Preflight:
 same filesystem: yes
 target exists: no
```

用户确认后才能 Apply。

---

# 14. 删除、回收站与 Undo

默认不提供“一键直接永久删除”。

## 14.1 删除重复文件

必须：

```text
Select
→ Preview
→ Explicit Confirm
→ Trash
```

## 14.2 Trash

每个 Library Root 使用自己的：

```text
.meloark-trash/
```

优先保持同一文件系统。

记录：

- 原路径
- Trash 路径
- 时间
- 原 inode/hash

支持 Restore。

永久清空 Trash 是独立操作，并需要再次确认。

## 14.3 Undo

首版要求可撤销：

- Tag Edit
- Rename
- Move
- Trash

Hardlink 创建可通过删除新增的 link 路径撤销。

---

# 15. Task Center

扫描、Hash、Fingerprint、刮削、歌词、整理都必须进入统一 Job 系统。

## 15.1 Job 状态

```text
queued
running
paused
cancel_requested
cancelled
completed
completed_with_errors
failed
```

## 15.2 Job Item 状态

每首文件独立保存状态：

```text
pending
running
success
skipped
failed
```

失败记录：

- error code
- message
- retryable
- attempt count

## 15.3 功能

Web 页面显示：

- 当前任务
- 总进度
- 当前文件
- 成功数
- 失败数
- 跳过数
- 速度
- ETA
- Pause
- Resume
- Cancel
- Retry Failed

## 15.4 重启恢复

Job 必须持久化到 SQLite。

容器重启后：

- `running` 转为 `interrupted`
- 用户可 Resume
- 已完成 item 不重复执行

## 15.5 并发

不同 Worker Pool：

```text
filesystem IO
hashing
fingerprint
provider HTTP
ffmpeg
```

用 Semaphore 控制并发。

默认参数必须偏保守，避免把 NAS IO 打满。

---

# 16. Web 播放

第一版保持简单。

## 16.1 Player

底部固定 Mini Player：

- Play / Pause
- Previous / Next
- Seek
- Volume
- Queue
- Shuffle
- Repeat
- Cover
- Title / Artist
- LRC 滚动歌词

## 16.2 浏览器原始播放

如果浏览器支持：

```text
GET /api/media/{id}/stream
```

必须支持 HTTP Range。

## 16.3 转码

浏览器不支持，或用户选择转码：

```text
FFmpeg
→ Opus/AAC
```

首版 Profile：

```text
original
opus-192
aac-256
mp3-320
```

实际浏览器默认优先 Opus/AAC。

## 16.4 转码缓存

Cache Key：

```text
media_file_id
source_size
source_mtime
profile
```

缓存位置：

```text
/data/cache/transcode/
```

可设置最大容量和 LRU 清理。

## 16.5 并发限制

FFmpeg 转码并发必须可配置。

默认：

```text
2
```

---

# 17. OpenSubsonic / Subsonic

MeloArk 自己作为 Server。

兼容路径：

```text
/rest/*.view
```

Web 自有 API 与 OpenSubsonic API 分开维护。

## 17.1 第一阶段兼容目标

系统：

```text
ping
getLicense
getOpenSubsonicExtensions
```

浏览：

```text
getMusicFolders
getIndexes
getMusicDirectory
getArtists
getArtist
getAlbum
getSong
```

列表：

```text
getAlbumList2
getRandomSongs
getStarred2
```

搜索：

```text
search3
```

媒体：

```text
stream
download
getCoverArt
```

播放状态：

```text
scrobble
getNowPlaying
```

收藏：

```text
star
unstar
```

Playlist：

```text
getPlaylists
getPlaylist
createPlaylist
updatePlaylist
deletePlaylist
```

歌词：

```text
getLyricsBySongId
```

并声明支持的 OpenSubsonic Extensions。

## 17.2 鉴权

至少支持客户端常见的：

- username
- salt + token
- JSON/XML response format

日志必须脱敏：

```text
password
token
salt
api key
```

不要把 OpenSubsonic query auth 原样写进 access log。

## 17.3 兼容测试

优先实际测试：

- Symfonium
- 至少一个其他 OpenSubsonic 客户端

同时根据 OpenSubsonic 官方 OpenAPI / 文档做契约测试。

---

# 18. 内部 REST API

以下为方向，不要求路径绝对不能调整，但应保持资源化。

## Auth

```text
POST /api/auth/setup
POST /api/auth/login
GET  /api/auth/profile
```

首个账号即 Admin。

公开部署后不允许再次 setup。

## Libraries

```text
GET    /api/libraries
POST   /api/libraries
PATCH  /api/libraries/{id}
DELETE /api/libraries/{id}
POST   /api/libraries/{id}/scan
POST   /api/libraries/{id}/preflight
```

## Tracks

```text
GET /api/tracks
GET /api/tracks/{id}
GET /api/tracks/{id}/files
```

支持：

- server-side pagination
- sort
- filter
- search
- missing-tag filters
- missing-lyrics filters

## Tags

```text
POST /api/tags/preview
POST /api/tags/apply
```

## Scrape

```text
POST /api/scrape/search
POST /api/scrape/batch
POST /api/scrape/apply
```

## Lyrics

```text
GET  /api/tracks/{id}/lyrics
POST /api/lyrics/search
POST /api/lyrics/preview
POST /api/lyrics/apply
```

## Duplicates

```text
POST /api/duplicates/analyze
GET  /api/duplicates/groups
GET  /api/duplicates/groups/{id}
POST /api/duplicates/actions/preview
POST /api/duplicates/actions/apply
```

## Organizer

```text
POST /api/organizer/preview
POST /api/organizer/apply
```

## Jobs

```text
GET  /api/jobs
GET  /api/jobs/{id}
POST /api/jobs/{id}/pause
POST /api/jobs/{id}/resume
POST /api/jobs/{id}/cancel
POST /api/jobs/{id}/retry-failed
```

## Realtime

优先 SSE：

```text
GET /api/events
```

用于：

- task progress
- scan changes
- playback state

首版没有双向实时需求时不必为了“炫酷”强行 WebSocket。

## Streaming

```text
GET /api/media/{id}/stream
GET /api/media/{id}/transcode
GET /api/artwork/{id}
```

---

# 19. 前端页面

## 19.1 Dashboard

定位：

> 音乐播放器视觉 + 管理健康中心，两者融合，但管理优先。

展示：

- 曲目数量
- Artist 数
- Album 数
- 总容量
- 格式分布
- 缺失 Tag
- 缺失 Lyrics
- 缺失 Cover
- 疑似重复
- Exact Duplicate
- 最近扫描
- 正在执行任务
- 最近播放
- 最近加入

背景可使用当前专辑封面的低强度 Blur。

## 19.2 Library

两种视图：

- Table
- Album Art Grid

Table：

- 虚拟滚动 / 服务端分页
- Column toggle
- Multi-select
- Batch Action
- 快捷 Filter

列：

```text
Cover
Title
Artist
Album
Year
Format
Quality
Duration
Size
Lyrics
Tag Health
Path
```

## 19.3 Track Drawer / Detail

右侧大 Drawer：

Tabs：

```text
Overview
Tags
Files
Lyrics
Artwork
Scrape
History
```

同一 Track 下可以看到多个 MediaFile Variant。

## 19.4 Scraper Workspace

左右对比：

```text
Local
vs
Provider Candidate
```

字段逐项 Diff。

## 19.5 Duplicate Center

按分析维度切 Tab：

```text
Hardlink Alias
Binary Exact
Audio Duplicate
Same Song Variants
Possible Duplicate
```

每个 Group 卡片展示：

```text
Cover
Title
Version
Codec
Bitrate
Sample Rate
Bit Depth
Size
Quality Score
Fingerprint Similarity
Path
```

## 19.6 Lyrics Center

显示：

- Local
- Provider candidates
- Quality score
- synced
- bilingual
- preview

支持 LRC 跟随试听。

## 19.7 Organizer

流程式 UI：

```text
Template
→ Source
→ Target
→ Preflight
→ Preview
→ Apply
```

## 19.8 Tasks

全局任务中心。

## 19.9 Settings

Tabs：

```text
General
Libraries
Metadata Providers
Lyrics Providers
AI
Organizer
Playback
OpenSubsonic
Jobs
Storage
Security
```

---

# 20. UI / UX 视觉方向

默认 Dark。

视觉参考方向：

- Apple Music
- Plexamp
- 现代 NAS 管理工具

但不是直接复制。

## 20.1 设计语言

- 深灰/近黑背景
- 蓝、紫、青品牌高光
- 玻璃层只用于 Hero / Player / Drawer
- 管理表格保持高信息密度
- Album Cover 是主视觉素材
- 轻量 Gradient
- Hover 微缩放
- Drawer / Dialog 平滑进入
- Skeleton
- Command Palette
- Toast
- Context Menu

## 20.2 性能约束

禁止：

- 大面积实时 Canvas 粒子
- 全页高强度 backdrop-filter
- 大量永久运行 CSS animation
- 为视觉引入大型 Three.js

“炫酷”不能牺牲 5 万～20 万曲目场景的可用性。

---

# 21. 搜索

SQLite FTS5。

搜索对象：

- Track
- Artist
- Album
- Path

中文优先支持：

- 原始中文
- 简繁归一后的关键词
- 拼音首字母辅助索引

例如：

```text
周杰伦
zhoujielun
zjl
```

至少后两种作为可选辅助字段，不改变原始数据。

---

# 22. 配置

保留 starter YAML 配置。

推荐：

```yaml
app:
  host: 0.0.0.0
  port: 31000

database:
  path: /data/meloark.db

storage:
  cache_path: /data/cache
  trash_folder_name: .meloark-trash

scan:
  schedule: "0 */6 * * *"
  watch: true
  concurrency: 4

jobs:
  io_workers: 2
  cpu_workers: 2
  provider_workers: 4
  ffmpeg_workers: 2

organizer:
  mode: hardlink
  template: "{artist}/{album}/{track:02} - {title}.{ext}"

ai:
  enabled: false
```

Secret：

- JWT secret
- Provider key
- AI key

支持环境变量 override。

Web UI 中 Secret 回显必须 mask。

---

# 23. 安全

## 23.1 Path Traversal

所有 Path：

1. canonicalize
2. 验证必须属于已配置 Library Root
3. 禁止用户 API 传绝对路径后直接执行

## 23.2 Symlink

默认：

```text
不跟随指向 Library Root 外部的 symlink
```

扫描和写入都要防逃逸。

## 23.3 登录

单管理员：

- First-run setup
- Argon2 password hash
- JWT
- Rate limit login
- Secure cookie 或 Bearer Token 保持 starter 一致方案

## 23.4 日志

绝不记录：

- password
- JWT
- Subsonic token
- Provider secret
- AI key

---

# 24. Provider 与开源许可证边界

参考项目：

- `xhongc/music-tag-web`
- `beetbox/beets`
- `minzgo/music-scraper`
- `HisAtri/LrcApi`

使用原则：

> 参考产品行为、数据流程和 UX，不直接复制 GPL 或具有附加限制项目的源码。

尤其：

- 不复制 music-tag-web 源码；
- 不复制 LrcApi 源码；
- 不把它们直接作为 Python 子模块打包进 MeloArk；
- Provider 采用 clean-room 重新实现；
- 对第三方 API 的访问逻辑独立模块化。

推荐 MeloArk 自身许可证：

```text
Apache-2.0
```

若最终希望更强的网络服务开源约束，可后续改 AGPL，但首版建议 Apache-2.0 方便生态使用。

构建镜像时维护：

```text
THIRD_PARTY_NOTICES.md
```

FFmpeg、Chromaprint 等 Runtime 依赖要记录其许可证。

---

# 25. 测试策略

## 25.1 Rust Unit

重点：

- Chinese normalize
- Version parser
- Filename parser
- Quality score
- Matching score
- Organizer template
- Path safety
- Duplicate grouping
- Lyrics score

## 25.2 Fixture Audio

仓库放极小测试文件：

```text
sample.mp3
sample.flac
sample.m4a
sample.ogg
sample.wav
```

以及：

- same audio transcoded
- same file copied
- hardlink fixture 在测试运行时临时创建

不要提交版权音乐。

## 25.3 Integration

测试：

- Scan
- Tag read/write
- Hash
- Fingerprint
- Organizer Dry Run
- Hardlink
- Retry
- Rollback
- Range stream
- FFmpeg transcode

## 25.4 Provider

不允许 CI 依赖真实 QQ/网易接口。

使用录制后的合法最小 Fixture / Mock Response。

真实 Provider 只做可选 smoke test。

## 25.5 Frontend

Vitest：

- Tag Diff
- Multi-select
- Duplicate group actions
- Task progress
- Player state

后期增加 Playwright E2E。

---

# 26. 性能设计

3TB 是真实目标场景。

## 26.1 不做的事情

禁止：

- 每次启动重新 Hash 3TB。
- 每次扫描重新生成全部 Fingerprint。
- 前端一次返回全部 Track。
- SQLite 一条 Track 一个独立事务提交。
- 无上限并发请求 Provider。
- 无上限开启 FFmpeg。

## 26.2 扫描优化

- Batch DB writes
- bounded channel
- incremental scan
- mtime / size fast path
- Cover thumbnail cache
- DB indexes
- FTS5
- server-side paging

## 26.3 Hash

BLAKE3。

先 candidate grouping，再 full read。

可提供：

```text
Full Library Exact Hash
```

但必须明确这是一个可能运行很久的低优先级任务。

---

# 27. Milestones

---

## M0 — Bootstrap / 基础工程

目标：项目可启动、可测试、可构建单镜像。

任务：

- [ ] 创建 monorepo
- [ ] 导入 react-starter
- [ ] 导入 rust-axum-starter
- [ ] 品牌替换为 MeloArk
- [ ] 清理 starter demo 页面
- [ ] 保留 starter 质量脚本
- [ ] 前后端真实 API 联通
- [ ] Axum ServeDir 托管 React
- [ ] Docker multi-stage
- [ ] amd64 / arm64 build
- [ ] `/api/health`
- [ ] first-run admin
- [ ] SQLite migration
- [ ] README / dev docs
- [ ] Logo 占位资产路径

验收：

```text
docker compose up
→ 打开 MeloArk
→ 首次创建管理员
→ 登录
→ Dashboard
```

---

## M1 — Library / Scan / Task Engine

目标：真正索引 NAS 曲库。

任务：

- [ ] Library CRUD
- [ ] Path preflight
- [ ] supported-format capability matrix
- [ ] Scanner
- [ ] Tag read
- [ ] ffprobe
- [ ] incremental scan
- [ ] inode/dev/hardlink detection
- [ ] watch
- [ ] scheduled reconciliation
- [ ] persistent jobs
- [ ] SSE task progress
- [ ] Library Table
- [ ] FTS search
- [ ] Dashboard health stats

验收：

- 可扫描大型目录；
- 重启后不重新读取未改变文件；
- UI 实时显示任务；
- 两个硬链接路径能识别为同一物理文件。

---

## M2 — Tag / Batch / Organizer

目标：安全修改和整理音乐。

任务：

- [ ] Track detail
- [ ] Tag editor
- [ ] batch editor
- [ ] find/replace
- [ ] regex
- [ ] Traditional → Simplified
- [ ] filename → tag parser
- [ ] tag → filename
- [ ] cover edit
- [ ] snapshot
- [ ] preview diff
- [ ] operation journal
- [ ] organizer template
- [ ] hardlink preflight
- [ ] hardlink apply
- [ ] path conflict
- [ ] rollback
- [ ] trash / restore

验收：

- 所有批量写操作有 Preview；
- 默认 Hardlink；
- 跨文件系统直接错误；
- 不会因为冲突覆盖目标文件；
- 失败项可 Retry。

---

## M3 — Scraper / Lyrics

目标：中文曲库刮削能力达到可用。

任务：

- [ ] Provider trait
- [ ] Provider health / cache / rate limit
- [ ] QQ
- [ ] NetEase
- [ ] Kugou
- [ ] Kuwo
- [ ] Migu
- [ ] MusicBrainz
- [ ] candidate merger
- [ ] confidence score
- [ ] version mismatch penalty
- [ ] scraper workspace
- [ ] artwork candidates
- [ ] lyrics providers
- [ ] LRC parser
- [ ] lyric quality score
- [ ] bilingual preview
- [ ] external LRC write
- [ ] embedded lyrics write
- [ ] batch scrape job

验收：

- 至少 3 个中文 Provider + MusicBrainz 稳定工作；
- 剩余 Provider 即使暂时 beta，也不能影响整体；
- 80～94 分候选不会自动写；
- 本地已有歌词不会静默覆盖。

---

## M4 — Duplicate / Fingerprint / AI

目标：完成本项目差异化核心能力。

任务：

- [ ] BLAKE3 jobs
- [ ] fpcalc jobs
- [ ] fingerprint similarity
- [ ] hardlink alias groups
- [ ] binary exact groups
- [ ] audio duplicate groups
- [ ] quality variants
- [ ] possible duplicate
- [ ] Quality Score
- [ ] version classifier
- [ ] duplicate UI
- [ ] bulk selection
- [ ] action Preview
- [ ] optional AI provider
- [ ] AI rerank
- [ ] AI duplicate explanation

验收：

测试集至少覆盖：

```text
同文件 copy
同文件 hardlink
FLAC → MP3 转码
Live
Remix
Remaster
Instrumental
不同码率同歌曲
```

未经用户确认不得删除任何一个文件。

---

## M5 — Player / FFmpeg / OpenSubsonic

目标：让 MeloArk 成为完整可用的家庭音乐服务器。

任务：

- [ ] bottom player
- [ ] play queue
- [ ] lyrics sync
- [ ] HTTP Range
- [ ] FFmpeg transcode
- [ ] transcode cache
- [ ] play history
- [ ] favorite
- [ ] playlist
- [ ] OpenSubsonic auth
- [ ] browsing APIs
- [ ] search3
- [ ] stream
- [ ] coverArt
- [ ] star
- [ ] playlist API
- [ ] lyrics extension
- [ ] extensions declaration
- [ ] Symfonium compatibility test

验收：

- Web 可播放普通音乐；
- 不支持的浏览器格式可自动转码；
- Symfonium 能登录、浏览、搜索、播放、显示封面与歌词。

---

## M6 — UI Polish / Release

目标：可公开 GitHub Release。

任务：

- [ ] Dashboard visual polish
- [ ] responsive
- [ ] Command Palette
- [ ] keyboard shortcuts
- [ ] Skeleton
- [ ] empty/error states
- [ ] performance profiling
- [ ] 50k+ fake metadata test
- [ ] security review
- [ ] Provider failure UX
- [ ] docs
- [ ] compose examples
- [ ] backup/restore docs
- [ ] THIRD_PARTY_NOTICES
- [ ] LICENSE
- [ ] GitHub Actions
- [ ] amd64 / arm64 image release

---

# 28. 第一版明确不做

避免 Codex 无限制扩范围：

- 在线音乐下载
- 在线音乐在线播放源
- 多用户 RBAC
- PostgreSQL
- 原生 iOS / Android
- 桌面客户端
- DLNA
- AirPlay Server
- 完整音乐推荐算法
- 音频编辑器
- 音频母带处理
- AI 听完整首歌后自动分类
- 无确认自动删除重复音乐
- 云端同步

---

# 29. 后续可扩展

- 多用户
- ReplayGain
- BPM / Key
- Composer / Lyricist / ISRC
- CUE 分轨
- DSD 高级 Tag
- 音频完整性扫描
- Lossless fake / spectral analyzer
- Smart Playlist
- Similarity Radio
- SonicSimilarity
- ListenBrainz
- Webhook
- Navidrome migration/import
- Jellyfin integration
- Plugin SDK

---

# 30. Codex 实现原则

Codex 必须遵守：

1. 先读完整 `plan.md`。
2. 严格按 M0 → M6 开发。
3. 每个 Milestone 完成后先测试，再继续。
4. 小问题自行作合理决策，不要频繁中断开发询问。
5. 不为了赶进度留下 TODO 假实现。
6. Provider 可以处于 beta，但核心本地功能不能是假数据。
7. 不复制参考 GPL 项目的实现代码。
8. 不擅自改变用户指定的 React / Rust starter。
9. 不降级 starter 的 Rust/Node/React 栈来规避问题。
10. 任何删除、覆盖、批量写操作都必须实现 Preview/Confirm。
11. 默认简体中文 UI。
12. 文件系统异常必须按 item 记录，不允许 panic。
13. Rust 继续禁止 unsafe。
14. 所有外部命令使用参数化 Process API，禁止字符串拼 Shell。
15. Provider Secret、JWT、Subsonic Token 必须脱敏。
16. 所有 Library path 操作必须防 Path Traversal。
17. 对 3TB 曲库按增量/按需计算设计，不能假设测试目录只有几十首歌。
18. 每阶段更新 `docs/IMPLEMENTATION_STATUS.md`。
19. 保持 OpenAPI 与前端类型一致。
20. Docker 是首要部署路径，开发模式仍应允许前后端独立运行。

---

# 31. Definition of Done

首版真正完成的标准不是“页面都存在”，而是：

```text
添加 NAS 曲库
→ 扫描
→ 找到缺失/错误 Tag
→ 多源刮削
→ 选歌词
→ 修改 Tag
→ 检测重复
→ 比较质量/版本
→ Dry Run
→ Hardlink 整理
→ Web 播放
→ Symfonium 连接 MeloArk
```

整个流程可以真实工作，并且任何危险操作都不会在用户不知情的情况下修改或删除源文件。
