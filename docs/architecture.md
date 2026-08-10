# 架构概览

MeloArk 首版是模块化单体：React 构建产物由 Axum 静态托管，业务数据与持久化任务保存在 `/data/meloark.db`，缓存位于 `/data/cache`。生产环境只交付一个 `linux/amd64` 镜像。

核心领域始终分离 Logical Track 与 Physical MediaFile。同一首逻辑歌曲可关联多个编码、采样率或发行版本的物理文件，避免把质量变体误判为应删除的重复项。

所有文件系统入口都从 Library Root 开始：保存时 canonicalize，执行时再次验证路径仍属于 Root；外部程序只通过参数化 Process API 调用。

## M1 扫描数据流

扫描任务先持久化到 `jobs` / `job_items`。文件枚举运行在阻塞线程中，通过容量为 64 的 bounded channel 逐项送给扫描 Worker，避免把大曲库全部路径一次载入内存。每个文件先比较 `path + size + mtime + dev + inode`；未变化的文件只更新本轮 seen 标记，不重新解析 Tag 或运行 ffprobe。

Lofty 负责 Tag 与基础时长，ffprobe 通过参数化 Process API 补充编码参数。基础扫描不计算 BLAKE3 或 Chromaprint。Track 与 MediaFile 分表保存，同一 Track 可以关联多个物理格式；`dev + inode` 被保留用于 Hardlink Alias 判断。

FTS 索引同时保存 NFKC、繁体转简体、标点/空白标准化结果，以及中文歌手的全拼和拼音首字母别名。索引版本写入运行时元数据；升级后只执行一次关系数据重建，避免每次启动全表重算。

任务进度通过带 Bearer Header 的 SSE 返回。JWT 不进入 query string，因此不会出现在 access log URI 中。
