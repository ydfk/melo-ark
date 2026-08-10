# 开发环境

## 工具链

- Web：Node.js 26.5.0、pnpm 11.17.0。
- Server：Rust 1.97.1、Rust 2024。
- Runtime：FFmpeg / ffprobe、Chromaprint / fpcalc。

建议使用两个 starter 自带的 `mise.toml` 安装精确版本。macOS 上也可以使用 `/Users/liyuhang/.cargo/bin/cargo` 调用现有 Rust 工具链。

## 配置

服务先读取 `apps/server/config/config.yaml`，再读取可选的 `config.local.yaml`，最后读取 `MELOARK__` 前缀的环境变量。嵌套字段使用双下划线，例如：

```bash
MELOARK__DATABASE__PATH=/tmp/meloark.sqlite \
MELOARK__JWT__SECRET=replace-with-a-long-random-secret \
cargo run
```

Secret 不得提交到仓库，也不会被服务写入日志。

## 质量门禁

```bash
cd apps/server
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release --locked

cd ../web
pnpm format:check
pnpm lint
pnpm test:run
pnpm build
```

生产交付还需在仓库根目录执行：

```bash
docker build --platform linux/amd64 -t meloark:local .
```
