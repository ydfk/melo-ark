# Provider 运行与合规边界

MeloArk 只获取用户主动发起检索所需的元数据、封面和歌词，不下载音乐音频。每个 Provider 都经过可配置 timeout、有限次瞬时错误重试、response cache、rate limit、circuit breaker 与 health state；单个来源异常不会中断聚合搜索。

## 稳定层级

- `musicbrainz`：公开、文档化的 Web Service 2 API；默认每秒最多一次请求，并发送包含应用名和版本的 User-Agent。
- `qq`、`netease`、`kugou`：独立的 clean-room 响应适配器，使用公开网页客户端可访问的接口。它们没有面向通用客户端的稳定公开目录 API，因此 “stable” 表示 MeloArk 具备完整适配器、fixture、降级与故障隔离，不表示上游承诺协议稳定。
- `kuwo`、`migu`、`external_lrc`：Beta 且默认关闭；配置 endpoint 前不会发出请求。

## 安全策略

- Provider base URL 必须为 HTTPS；只有测试用 localhost 可以使用 HTTP。
- 只有 timeout 和 HTTP 暂时性失败会在限流约束下执行有限次退避重试；解析、认证和业务错误不会盲目重试。
- 封面下载限制为 10 MiB，之后仍由 Lofty 校验实际图片格式。
- Provider 只返回候选，不写文件。
- 元数据候选必须经过 Diff 和用户确认；歌词已有内容时默认冲突。
- Provider 原始响应只保存在 SQLite 缓存中并按 TTL 过期，不写入音乐目录。
