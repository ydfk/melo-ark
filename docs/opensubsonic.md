# OpenSubsonic 兼容说明

MeloArk 在 `/rest/*.view` 提供独立于 Web API 的 OpenSubsonic/Subsonic 接口。

## 客户端配置

- Server URL：`http(s)://HOST:31000`
- Username / Password：首次初始化时创建的管理员凭据
- API version：`1.16.1`
- 推荐认证：salt + token；同时兼容明文和 `enc:` 密码形式

建议通过 HTTPS 反向代理公开服务。OpenSubsonic query 参数不会写入 access log，日志只记录 URL path。

## 已实现接口

系统、浏览、专辑列表、随机列表、收藏、`search3`、`stream`、`download`、封面、scrobble、Now Playing、歌单 CRUD，以及 `getLyricsBySongId`。`search3` 与 Web 共用 FTS 标准化，可按中文、全拼和拼音首字母搜索；中文艺术家索引按拼音首字母分组。服务声明 `songLyrics` 与 `formPost` extensions。

转码由 `maxBitRate` 选择 Opus 192、AAC 256 或 MP3 320 Profile；不传时优先原始文件并支持单段 HTTP Range。

## 兼容性证据

自动化测试使用 Symfonium 风格的 `u/t/s/v/c/f` 参数覆盖 JSON/XML 登录、浏览、搜索、歌曲详情、Range 播放、错误码、歌单和歌词契约。该测试不是实体 Android 设备实测；正式发布前仍应使用当前 Symfonium 与另一个 OpenSubsonic 客户端完成外部设备验收。

客户端无法登录时，先确认 JWT Secret 未在初始化后更换，再查看 `/api/health`。连续失败会触发一分钟登录限流。
