# 备份与恢复

## 需要备份的内容

必须备份整个 `/data` Volume，至少包括：

- `meloark.db`、`meloark.db-wal`、`meloark.db-shm`；
- `/data/cache`（可选，丢失后会重建）；
- Compose、`.env` 和自定义配置。

音乐文件不在 `/data` 内，必须由 NAS 自身的快照或备份策略保护。MeloArk 的操作日志和 `.meloark-trash` 不能替代音乐备份。

## 一致性备份

最稳妥的方式是短暂停止容器后复制整个数据目录：

```bash
docker compose stop meloark
tar -C ./data -czf "meloark-data-$(date +%Y%m%d-%H%M%S).tar.gz" .
docker compose start meloark
```

如果必须在线备份，应使用 SQLite Backup API 或先执行 WAL checkpoint；不要只复制主数据库而遗漏仍在 WAL 中的提交。

容器运行时不要从 macOS/Windows 宿主机直接用 `sqlite3` 打开 bind-mounted 数据库；跨虚拟化文件锁可能破坏正在运行的 WAL 会话。需要检查数据库时先停止容器，或把 `/data` 放在 Docker named volume 中并通过同一 Linux VM 内的工具执行备份。

## 恢复

```bash
docker compose down
mv ./data ./data.before-restore
mkdir ./data
tar -C ./data -xzf meloark-data-YYYYMMDD-HHMMSS.tar.gz
docker compose up -d
curl --fail http://127.0.0.1:31000/api/health
```

恢复后检查管理员登录、Library Root 容器路径、任务中心和随机曲目播放。若宿主机挂载路径改变，只更新 Compose 映射；Web 内部路径保持不变可避免重新配置。
