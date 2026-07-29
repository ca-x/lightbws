# LightBWS

LightBWS 是一个可持久化、自托管的 Bitwarden Secrets Manager server。单个发布二进制同时包含 Axum 和 SeaORM 后端、内嵌的 React/Astryx 管理界面、SDK 兼容接口、加密导入导出，以及定时远程备份。

[English](README.md)

## 界面截图

![LightBWS 登录界面](screenshoot/login-zh.png)

![LightBWS 控制台](screenshoot/dashboard-zh.png)

## 功能

- 使用 SQLite 持久化数据，启用 WAL、外键和并发访问保护。
- 支持通过环境变量初始化管理员，并在 Web 界面管理用户。
- 支持项目、密钥、机器账号、软删除回收站和一次性访问令牌展示。
- 使用 Cookie 会话、CSRF 防护、Argon2id 密码、加密备份凭据和安全响应头。
- Web 界面支持中文和英文，提供 7 个 Astryx 内置主题，以及跟随系统、浅色和深色模式。
- 支持使用口令加密的可移植导入导出。
- 支持将加密备份定时写入兼容 S3 的存储和 WebDAV。
- 前端已嵌入每个发布二进制，不需要单独的 Web 服务器。
- 提供 Linux GNU/musl、macOS、Windows 发布压缩包，以及多架构 GHCR 和 Docker Hub 镜像。

## 快速开始

### Docker

```bash
docker run --name lightbws --restart unless-stopped \
  -p 127.0.0.1:8080:8080 \
  -v lightbws-data:/data \
  -e LIGHTBWS_ADMIN_USERNAME=admin \
  -e LIGHTBWS_ADMIN_PASSWORD='replace-with-a-long-password' \
  ghcr.io/ca-x/lightbws:latest
```

打开 `http://127.0.0.1:8080`。文档中的 Docker 默认配置只监听本机回环地址。远程访问时，请通过 HTTPS 反向代理访问 LightBWS，并设置 `LIGHTBWS_COOKIE_SECURE=true`。不要把 HTTP 端口直接暴露到不可信网络。

仓库内提供了可直接使用的 `docker-compose.yml`：

```bash
cp .env.example .env
# 编辑 .env，设置唯一且足够长的 LIGHTBWS_ADMIN_PASSWORD。
docker compose up -d
docker compose logs -f lightbws
```

如果 `LIGHTBWS_ADMIN_PASSWORD` 为空，Compose 会拒绝启动，因此公开模板不会被用作已安装实例的管理员密码。命名卷 `lightbws-data` 会在容器升级后继续保留 SQLite 数据库和生成的 `master.key`。

镜像会同时发布到 `ghcr.io/ca-x/lightbws` 和 `docker.io/czyt/lightbws`。如需通过 Docker Hub 拉取，可在 `.env` 中设置 `LIGHTBWS_IMAGE=docker.io/czyt/lightbws:latest`。

更新到最新发布镜像：

```bash
docker compose pull
docker compose up -d
```

如果部署需要固定版本，可在 `.env` 中设置 `LIGHTBWS_IMAGE=ghcr.io/ca-x/lightbws:0.1.0`。执行 `docker compose down` 会保留数据卷，执行 `docker compose down -v` 会永久删除数据卷。

### 发布二进制

从 GitHub Releases 下载当前平台的压缩包，然后运行：

```bash
export LIGHTBWS_DATA_DIR=./data
export LIGHTBWS_ADMIN_USERNAME=admin
export LIGHTBWS_ADMIN_PASSWORD='replace-with-a-long-password'
./lightbws
```

管理员环境变量仅在数据库为空时使用。已有数据库不会再次从环境变量初始化。

## 配置

`.env.example` 的前三项只用于配置 Docker Compose，LightBWS 进程不会直接读取它们：

| Compose 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `LIGHTBWS_IMAGE` | `ghcr.io/ca-x/lightbws:latest` | Compose 拉取的容器镜像。Docker Hub 对应地址为 `docker.io/czyt/lightbws:latest`。 |
| `LIGHTBWS_LISTEN_ADDRESS` | `127.0.0.1` | 发布端口绑定的宿主机地址。除非通过 HTTPS 反向代理或可信网络访问，否则应保留回环地址。 |
| `LIGHTBWS_PORT` | `8080` | 映射到容器内 `8080` 端口的宿主机端口。 |

其余变量会传入 LightBWS 容器：

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `LIGHTBWS_BIND` | `0.0.0.0:8080` | HTTP 监听地址。 |
| `LIGHTBWS_DATA_DIR` | `data` | SQLite 数据库和生成的主密钥目录。 |
| `LIGHTBWS_ADMIN_USERNAME` | 无 | 初始管理员用户名。 |
| `LIGHTBWS_ADMIN_PASSWORD` | 无 | 初始管理员密码，至少 8 个字符。 |
| `LIGHTBWS_COOKIE_SECURE` | `false` | 要求 Web 会话 Cookie 只通过 HTTPS 传输。使用 HTTPS 反向代理时启用。 |
| `LIGHTBWS_ENABLE_UPSTREAM_COMPATIBILITY_ACCOUNT` | `false` | 创建上游 SDK 测试夹具使用的公开固定凭据，仅用于兼容性测试。不要在共享或面向互联网的部署中启用。 |
| `LIGHTBWS_MASTER_KEY` | 自动生成 | 用于加密存储备份凭据的 Base64url 或十六进制 32 字节密钥。 |
| `RUST_LOG` | `lightbws=info,tower_http=info` | 结构化日志过滤器。 |

镜像已经设置 `LIGHTBWS_BIND=0.0.0.0:8080` 和 `LIGHTBWS_DATA_DIR=/data`。原生二进制部署可以覆盖这两个运行时变量；Compose 部署通常应通过 `LIGHTBWS_LISTEN_ADDRESS` 和 `LIGHTBWS_PORT` 调整宿主机映射。

如果未设置 `LIGHTBWS_MASTER_KEY`，LightBWS 会在数据目录创建仅所有者可读的 `master.key`。请将它与 SQLite 数据库一起持久化，否则无法恢复远程备份文件。

## SDK 和 BWS

LightBWS 实现官方 SDK 使用的 Secrets Manager 路由，并持久化客户端发送的密文。请在 Web 界面创建机器账号，复制一次性凭据，然后将客户端指向 LightBWS 基础地址。凭据交换会签发随机的一小时 Bearer 令牌，并将摘要写入 SQLite。每次 SDK 请求都会检查令牌是否过期，以及机器账号是否仍处于有效状态。

正常部署应使用 Web 界面创建的机器账号。`LIGHTBWS_ENABLE_UPSTREAM_COMPATIBILITY_ACCOUNT` 只用于运行依赖公开固定客户端凭据的上游 SDK 测试夹具。它是测试兼容开关，不是生产认证模式。

```bash
export BWS_ACCESS_TOKEN='0.<client-id>.<client-secret>:X8vbvA0bduihIDe/qrzIQQ=='
bws --server-url http://127.0.0.1:8080 project list
```

仓库在 `demo/sdk-demo` 提供官方 Rust SDK 往返演示：

```bash
LIGHTBWS_URL=http://127.0.0.1:8080 \
BWS_ACCESS_TOKEN="$BWS_ACCESS_TOKEN" \
cargo run --manifest-path demo/sdk-demo/Cargo.toml
```

演示会完成认证，创建项目和密钥，读取并列出它们，然后删除测试数据。

SDK 创建的数据会以 Bitwarden 密文保存在 SQLite 中，并由 SDK 客户端解密。Web 创建的数据属于经过认证的 Web 信任边界，不会伪装成 SDK 密文响应。界面会明确标记 SDK 数据。

### Fnox

Fnox 的 Bitwarden Secrets Manager provider 会调用本机安装的 `bws` CLI。请先在 LightBWS 中创建项目和 SDK 密钥，再配置项目 ID 与密钥名称：

```toml
[providers]
bws = { type = "bitwarden-sm", project_id = "your-project-id" }

[secrets]
DATABASE_URL = { provider = "bws", value = "database-url" }
```

```bash
export BWS_ACCESS_TOKEN='<机器账号访问令牌>'
export BWS_SERVER_URL='https://lightbws.example.com'
fnox get DATABASE_URL
fnox exec -- npm start
```

## 备份和迁移

- 手动导出使用独立的口令派生 Argon2id 密钥，可在不同 LightBWS 实例之间迁移。
- S3 和 WebDAV 定时快照使用持久化的实例主密钥加密。
- 远程凭据使用 AES-256-GCM 加密保存在 SQLite 中，API 永远不会返回这些凭据。
- 备份地址必须使用 HTTPS，并且只能解析到公网 IP。程序禁用重定向以降低 SSRF 风险。
- S3 上传使用 AWS Signature Version 4。WebDAV 上传会先创建所需目录，再使用 `PUT` 上传。
- 导出快照在事务中保持一致，明文大小限制为 64 MiB，以控制内存占用。

## 集成

- [官方 SDK](https://github.com/bitwarden/sdk-sm)
- [BWS CLI](https://github.com/bitwarden/sdk-sm/tree/main/crates/bws)
- [Fnox Bitwarden Secrets Manager provider](https://fnox.jdx.dev/providers/bitwarden-sm)
- [Bitwarden Secrets Manager 帮助文档](https://bitwarden.com/help/secrets-manager-overview/)

## 开发

```bash
npm --prefix web ci --ignore-scripts
npm --prefix web run dev

# 在另一个终端执行
LIGHTBWS_ADMIN_USERNAME=admin \
LIGHTBWS_ADMIN_PASSWORD=development-password \
cargo run
```

Vite 会将 `/api` 代理到 `127.0.0.1:8080`。发布构建需要前端包，因为它通过 `rust-embed` 嵌入二进制：

```bash
npm --prefix web run build
cargo build --release
```

无需启动服务器即可检查 SDK 演示：

```bash
cargo check --manifest-path demo/sdk-demo/Cargo.toml
```

## 测试

发布门禁会执行前端类型检查、单元测试和生产构建，Rust 格式检查、Clippy、全目标测试和 release 构建，SDK 演示编译检查，工作流验证，以及依赖安全审计：

```bash
npm --prefix web ci --ignore-scripts
npm --prefix web run typecheck
npm --prefix web run test:ci
npm --prefix web run build
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --release
cargo check --manifest-path demo/sdk-demo/Cargo.toml
actionlint .github/workflows/*.yml
cargo audit
npm --prefix web audit
```

运行时验收还会覆盖官方 Rust SDK 的创建、读取、列表和删除往返流程，`bws` 与 Fnox 按项目读取密钥，使用 `agent-browser` 检查管理员和用户管理流程、浏览器控制台，以及 Axe 无障碍扫描。

## 发布

`main` 通过 CI 后，Docker 工作流会发布 `latest` 和提交 SHA 镜像标签。推送 `vX.Y.Z` 语义化标签后，工作流会先校验所有包版本并重新运行完整测试。只有所有二进制和多架构镜像构建成功后，最后一个任务才会创建 GitHub Release：

```bash
git tag -a v0.1.0 -m "LightBWS v0.1.0"
git push origin main
git push origin v0.1.1
```

Docker 工作流会在 GitHub 原生 Runner 上分别构建 `linux/amd64` 和 `linux/arm64`，再将相同的多架构标签发布到 `ghcr.io/ca-x/lightbws` 和 `docker.io/czyt/lightbws`。仓库或组织需要提供 `DOCKERHUB_USERNAME` 和 `DOCKERHUB_TOKEN` 两个 secret。每个发布压缩包包含二进制、两种语言的 README 和许可证，并提供独立的 SHA-256 校验文件。前端文件已经嵌入二进制。
