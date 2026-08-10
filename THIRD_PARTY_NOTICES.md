# Third-Party Notices

MeloArk source code is licensed under Apache License 2.0. Its source and container image depend on third-party software distributed under their own licenses.

## Runtime programs included in the container

- **FFmpeg / ffprobe** — FFmpeg Project. The final `linux/amd64` image currently installs Debian Bookworm's unmodified FFmpeg package, whose `ffmpeg -buildconf` output includes `--enable-gpl` and GPL components such as x264/x265. Treat that packaged FFmpeg build as **GPL-2.0-or-later**. MeloArk invokes FFmpeg/ffprobe as independent command-line programs; the MeloArk source remains Apache-2.0.
- **Chromaprint / fpcalc** — AcoustID contributors. Chromaprint's own code is MIT, and upstream documents its ordinary combined work as LGPL-2.1 because of FFmpeg portions. The container's Debian `fpcalc` binary links the GPL-enabled Debian FFmpeg libraries described above, so treat this packaged runtime combination as **GPL-2.0-or-later** as well.
- **Debian Bookworm runtime packages** — their copyright files are available in `/usr/share/doc/*/copyright` inside the image.

## Application dependencies

Rust and JavaScript dependencies retain their respective licenses. The authoritative resolved versions are recorded in `apps/server/Cargo.lock` and `apps/web/pnpm-lock.yaml`. Release workflows produce an OCI SBOM for the published image.

MeloArk's clean-room Provider adapters do not copy or bundle source code from music-tag-web, LrcApi, beets, or music-scraper. Remote music metadata remains subject to each provider's terms and the user's local jurisdiction.
