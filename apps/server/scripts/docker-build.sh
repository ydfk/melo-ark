#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)
image_tag=${1:-meloark:local}

if ! command -v docker >/dev/null 2>&1; then
  echo "[ERROR] 未找到 Docker，请先安装并启动 Docker Desktop。" >&2
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  echo "[ERROR] Docker daemon 不可用，请先启动 Docker Desktop。" >&2
  exit 1
fi

cd "$root"
docker build --tag "$image_tag" .
