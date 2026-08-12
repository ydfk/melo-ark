# 安全设计与审查

更新日期：2026-08-10。

## 文件边界

- 所有媒体写入、Trash、Organizer、歌词、Hash、指纹和播放路径都从数据库中的 Library Root 与相对路径重建。
- 操作前 canonicalize 源与目标，并验证结果仍属于对应 Library Root；指向 Root 外部的 symlink 会被拒绝。
- 外部 `ffprobe`、`ffmpeg` 与 `fpcalc` 使用 `Command` 参数列表，不经过 shell。
- Organizer 只创建 Hardlink，不在跨文件系统时回退 Copy；冲突不覆盖。
- 永久清理是独立持久化操作，必须再次提交精确确认；Preview 与 Apply 都会验证目标是回收站内的普通文件，并核对 size/dev/inode。符号链接、路径逃逸或预览后变化的文件只记录逐项失败，不会删除。
- 永久清理只调用单文件删除，不递归删除回收站目录。

## 鉴权与 Secret

- 管理员密码使用 Argon2；JWT 使用 HS256，生产 Secret 至少 32 个随机字符。
- 登录失败按用户名执行 5 次/分钟限流，并限制限流表规模。
- OpenSubsonic 原密码只以 AES-256-GCM 密文保存在 SQLite 中；token 使用常量时间比较。
- Web 播放使用 10 分钟、绑定单个 MediaFile 的短期 JWT，不在 audio 元素中暴露管理员 JWT。
- AI 默认关闭，只在用户提交 `SEND_METADATA` 后发送结构化元数据，不上传音频文件。

## 日志与 HTTP

- HTTP span 只记录 method 与 path，不记录 query、Authorization、Cookie 或 request body。
- 响应统一添加 `X-Content-Type-Options: nosniff`。
- Web 静态资源不加载远程字体或 CDN；只有用户主动使用已启用 Provider/AI 时才会向对应 endpoint 发起外部请求。
- 推荐由 Caddy、Traefik 或其他反向代理提供 HTTPS、安全 Header 与可信网络入口。

## 已知边界

- 首版为单管理员，不提供 RBAC。
- 进程内登录限流在重启后清空；公网部署还应在反向代理层增加 IP 级限流。
- CORS 为 HomeLab 兼容性允许任意 Origin；Bearer Token 不使用 Cookie 自动携带。公网部署可进一步收紧 Origin。
- 依赖与镜像应由 GitHub Actions 定期重建；发布前检查 Rust、pnpm 和 Alpine Linux 安全公告。
