# ADR 0002：持久化增量扫描与有界枚举

- 状态：已接受
- 日期：2026-08-10

## 决定

扫描、后续 Hash、Fingerprint、Provider、Organizer 共用 SQLite `jobs` / `job_items` 状态机。文件枚举通过 bounded channel 流入 Worker，不使用内存 Vec 保存完整目录。基础扫描以 `relative_path + size + mtime + dev + inode` 判断增量，只在变化时读取 Tag 与 ffprobe。

Watch 仅作为低延迟触发器；定时 reconcile 才负责最终一致性。容器重启时 `running` 任务转为 `interrupted`，用户恢复后跳过已完成 item。

## 影响

进度可以暂停、恢复、取消并通过 SSE 实时展示。基础扫描不会读取整份 3TB 曲库来计算 Hash 或 Fingerprint，这两类任务留在 M4 按需执行。
