# LightBWS

LightBWS is a persistent, self-hosted implementation of the Bitwarden Secrets Manager fake-server contract. It combines an Axum/SeaORM backend, an embedded React/Astryx administration interface, official SDK-compatible endpoints, encrypted import/export, and scheduled remote backups in one release binary.

LightBWS 是一个可持久化、自托管的 Bitwarden Secrets Manager fake-server 兼容实现。单个发布二进制内同时包含 Axum/SeaORM 后端、React/Astryx 管理界面、官方 SDK 兼容接口、加密导入导出与远程定时备份。

## Features / 功能

- Persistent SQLite database with WAL, foreign keys, and safe concurrent access
- Administrator bootstrap from environment variables and Web user management
- Projects, secrets, machine accounts, soft-delete trash, and one-time access-token display
- Cookie sessions, CSRF protection, Argon2id passwords, encrypted backup credentials, and hardened response headers
- Chinese and English UI with Astryx system/light/dark theme modes
- Portable passphrase-encrypted import/export
- Scheduled encrypted backups to S3-compatible storage and WebDAV
- Frontend embedded into every release binary; no separate Web server is required
- Linux GNU/musl, macOS, and Windows release archives plus multi-architecture GHCR images

## Quick start / 快速开始

### Docker

```bash
docker run --name lightbws --restart unless-stopped \
  -p 127.0.0.1:8080:8080 \
  -v lightbws-data:/data \
  -e LIGHTBWS_ADMIN_USERNAME=admin \
  -e LIGHTBWS_ADMIN_PASSWORD='replace-with-a-long-password' \
  ghcr.io/ca-x/lightbws:latest
```

Open `http://127.0.0.1:8080`. The documented Docker defaults bind only to loopback. For remote access, keep LightBWS behind an HTTPS reverse proxy and set `LIGHTBWS_COOKIE_SECURE=true`; do not publish its HTTP port directly to an untrusted network.

默认 Docker 配置仅监听本机回环地址。远程访问时请通过 HTTPS 反向代理暴露服务，并设置 `LIGHTBWS_COOKIE_SECURE=true`，不要把 HTTP 端口直接发布到不可信网络。

仓库内提供了可直接使用的 `docker-compose.yml`。首次启动前复制环境变量模板并设置管理员密码：

```bash
cp .env.example .env
# Edit .env and set a unique LIGHTBWS_ADMIN_PASSWORD.
docker compose up -d
docker compose logs -f lightbws
```

Compose refuses to start while `LIGHTBWS_ADMIN_PASSWORD` is empty, so the public template can never become the installed administrator password. The named volume `lightbws-data` persists the SQLite database and generated `master.key` across container upgrades.

如果没有设置 `LIGHTBWS_ADMIN_PASSWORD`，Compose 会直接拒绝启动。命名卷 `lightbws-data` 会在容器升级后继续保留 SQLite 数据库与自动生成的 `master.key`。

To upgrade to the newest published image:

```bash
docker compose pull
docker compose up -d
```

Set `LIGHTBWS_IMAGE=ghcr.io/ca-x/lightbws:0.1.0` in `.env` when a deployment must remain pinned to a specific release. Running `docker compose down` keeps the data volume; `docker compose down -v` permanently deletes it.

如需固定版本，可在 `.env` 中设置 `LIGHTBWS_IMAGE=ghcr.io/ca-x/lightbws:0.1.0`。`docker compose down` 会保留数据卷，而 `docker compose down -v` 会永久删除数据。

### Release binary / 发布二进制

Download the archive for the current platform from GitHub Releases, then run:

```bash
export LIGHTBWS_DATA_DIR=./data
export LIGHTBWS_ADMIN_USERNAME=admin
export LIGHTBWS_ADMIN_PASSWORD='replace-with-a-long-password'
./lightbws
```

The administrator variables are required only when the database is empty. Existing databases are never reinitialized from environment variables.

管理员环境变量仅在空数据库首次启动时使用，后续启动不会覆盖数据库中的用户。

## Configuration / 配置

| Variable | Default | Purpose |
| --- | --- | --- |
| `LIGHTBWS_BIND` | `0.0.0.0:8080` | HTTP listen address |
| `LIGHTBWS_DATA_DIR` | `data` | SQLite database and generated master-key directory |
| `LIGHTBWS_ADMIN_USERNAME` | none | Initial administrator username |
| `LIGHTBWS_ADMIN_PASSWORD` | none | Initial administrator password, minimum 8 characters |
| `LIGHTBWS_COOKIE_SECURE` | `false` | Require HTTPS for Web session cookies; enable behind an HTTPS reverse proxy |
| `LIGHTBWS_ENABLE_UPSTREAM_TOKEN` | `false` | Create the upstream fake-server's fixed compatibility machine credentials for local testing only |
| `LIGHTBWS_MASTER_KEY` | generated | Base64url or hexadecimal 32-byte key used to encrypt stored backup credentials |
| `RUST_LOG` | `lightbws=info,tower_http=info` | Structured log filter |

If `LIGHTBWS_MASTER_KEY` is not set, LightBWS creates `master.key` with owner-only permissions in the data directory. Persist this file together with the SQLite database; remote backup files cannot be recovered without it.

未设置 `LIGHTBWS_MASTER_KEY` 时，程序会在数据目录生成仅所有者可读的 `master.key`。请与数据库一起持久化，否则远程备份文件无法恢复。

## SDK and BWS / SDK 与 BWS

LightBWS preserves all routes used by the upstream fake server and persists the ciphertext sent by the official SDK. Create a machine account in the Web UI, copy its one-time credential, and point the client at the LightBWS base URL. Credential exchange issues a random one-hour bearer token whose digest is stored in SQLite; every SDK request checks expiry and the machine account's current revocation state.

```bash
export BWS_ACCESS_TOKEN='0.<client-id>.<client-secret>:X8vbvA0bduihIDe/qrzIQQ=='
bws --server-url http://127.0.0.1:8080 project list
```

An official Rust SDK round-trip demo is provided in `demo/sdk-demo`:

```bash
LIGHTBWS_URL=http://127.0.0.1:8080 \
BWS_ACCESS_TOKEN="$BWS_ACCESS_TOKEN" \
cargo run --manifest-path demo/sdk-demo/Cargo.toml
```

The acceptance demo authenticates, creates a project and secret, reads and lists them, then removes the test records.

SDK-created values remain Bitwarden ciphertext in SQLite and are decrypted by the SDK client. Web-created values use the authenticated Web trust boundary and are intentionally not exposed through SDK ciphertext responses. The UI labels SDK-owned records accordingly.

SDK 创建的数据会以 Bitwarden 密文持久化并由 SDK 客户端解密。Web 创建的数据属于独立的 Web 信任边界，不会伪装成 SDK 密文响应；界面会明确标记 SDK 数据。

## Backups and transfer / 备份与迁移

- Manual exports use an independent passphrase-derived Argon2id key and are portable between LightBWS installations.
- Scheduled S3/WebDAV snapshots are encrypted with the persistent instance master key.
- Remote credentials are AES-256-GCM encrypted in SQLite and are never returned by the API.
- Backup endpoints must use HTTPS and resolve only to public IP addresses; redirects are disabled to reduce SSRF risk.
- S3 uploads use AWS Signature Version 4. WebDAV uploads create required collections and then use `PUT`.
- Export snapshots are transactionally consistent and capped at 64 MiB of plaintext to bound memory use.

手动导出使用口令派生的独立密钥，可在不同实例间迁移；定时远程备份使用实例主密钥。远程凭据不会通过 API 返回。

## Integrations / 集成

- [Official SDK](https://github.com/bitwarden/sdk-sm)
- [BWS CLI](https://github.com/bitwarden/sdk-sm/tree/main/crates/bws)
- [Fnox Bitwarden Secrets Manager provider](https://fnox.jdx.dev/providers/bitwarden-sm)
- [Bitwarden Secrets Manager help](https://bitwarden.com/help/secrets-manager-overview/)

## Development / 开发

```bash
npm --prefix web ci --ignore-scripts
npm --prefix web run dev

# In another terminal
LIGHTBWS_ADMIN_USERNAME=admin \
LIGHTBWS_ADMIN_PASSWORD=development-password \
cargo run
```

Vite proxies `/api` to `127.0.0.1:8080`. A release build requires the frontend bundle because it is embedded with `rust-embed`:

```bash
npm --prefix web run build
cargo build --release
```

## Publishing / 发布

After CI succeeds on `main`, the Docker workflow publishes the `latest` and commit-SHA tags. Pushing a semantic `vX.Y.Z` tag first validates every package version and reruns the complete test suite; only after every binary and multi-architecture image succeeds does one final job create the GitHub Release:

```bash
git tag -a v0.1.0 -m "LightBWS v0.1.0"
git push origin main
git push origin v0.1.0
```

The Docker workflow publishes `linux/amd64` and `linux/arm64` images to `ghcr.io/ca-x/lightbws`. Each release archive contains the binary, README, and license, with a companion SHA-256 checksum asset; frontend files are already embedded in the binary.

推送 `main` 会发布 `latest` 与提交 SHA 镜像标签；推送语义化的 `v*` 标签会发布对应版本的多架构镜像，并为 Linux GNU/musl、macOS 和 Windows 创建 GitHub Release。每个发布压缩包包含二进制、README 和许可证，同时提供独立的 SHA-256 校验文件；前端已嵌入二进制。
