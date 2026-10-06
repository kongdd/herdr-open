# herdr-open

在 Herdr 中打开当前 workspace：本地 Zed / VS Code，或本机编辑器通过 SSH 打开服务器上的目录。Rust 单文件实现，仅依赖 `serde_json`；支持 macOS、Linux。

## 安装

需要 Rust/Cargo、Herdr ≥ 0.9.2。本机安装编辑器 CLI（`zed` 或 `code`）；VS Code 需要 Remote - SSH 扩展。远程连接推荐使用 `~/.ssh/config` 中的别名，端口、跳板和密钥都放在 SSH 配置中。

```sh
herdr plugin install kongdd/herdr-open
```

开发时：

```sh
cargo build --release --locked
herdr plugin link "$PWD"
```

`plugin link` 不会运行 build。独立 helper 安装到本机 PATH：

```sh
cargo install --git https://github.com/kongdd/herdr-open --locked
```

## 快捷键

在 Herdr 配置中添加（远程模式配置在服务器端）：

```toml
[[keys.command]]
key = "prefix+o"
type = "plugin_action"
command = "kongdd.open.open-zed"
description = "Open in Zed"

[[keys.command]]
key = "prefix+v"
type = "plugin_action"
command = "kongdd.open.open-code"
description = "Open in VS Code"
```

也可以手动触发：

```sh
herdr plugin action invoke kongdd.open.open-zed
herdr plugin action invoke kongdd.open.open-code
```

路径优先级：`worktree.checkout_path` → `workspace_cwd` → `focused_pane_cwd`。不使用插件自身 cwd 作为备用路径。

## 远程使用

1. 在服务器安装插件：`herdr plugin install kongdd/herdr-open`。
2. 确保 `ssh nas herdr --version` 能运行，服务器的非交互 shell PATH 必须包含 `herdr`。
3. 在本机通过 helper 启动：

```sh
herdr-open attach nas
# 透传其他 Herdr 参数
herdr-open attach nas -- --session work
```

随后按 `prefix+o` / `prefix+v`，本机执行：

```sh
zed ssh://nas/home/user/repo
code --remote ssh-remote+nas /home/user/repo
```

helper 创建仅监听 loopback 的临时 TCP relay 和独立 SSH ControlMaster 反向隧道，向远端插件配置目录写入随机 token 和端口。请求有 token 校验、大小限制和读写超时，不执行传入的 shell 命令。正常退出、Ctrl+C、SIGTERM、SIGHUP 时关闭隧道。SIGKILL 或机器崩溃无法执行清理；SSH keepalive 会检测断连。

默认远端端口 `47831`，同一服务器同一用户同时只能有一个 attach；冲突时明确报错。多用户服务器上可为每个用户选不同端口：

```sh
herdr-open attach nas --port 47832
```

远端 token 文件仅当前用户可读。SSH 服务必须允许反向转发；服务器应使用默认 `GatewayPorts no`，不要把 relay 转发端口暴露到公网。具有该用户权限的远程进程可以使用 relay，属于同一信任边界。

远程配置保留在插件配置目录中，断开后不会自动回退到服务器上的编辑器。重连会更新配置。如果这台服务器之后需要作为本地桌面使用，删除 relay 配置：

```sh
HERDR_PLUGIN_CONFIG_DIR="$(herdr plugin config-dir kongdd.open)" herdr-open reset
```

第一版使用显式 attach wrapper；普通 `herdr --remote nas` 不会自动建立 relay。暂不支持 Windows helper、多客户端共用同一远端用户及自动重连隧道。

## 独立命令和 CLI 覆盖

```sh
herdr-open local zed /absolute/path
herdr-open remote code nas /absolute/path
HERDR_OPEN_ZED_BIN=/custom/path/zed herdr-open attach nas
```

可覆盖 `HERDR_OPEN_ZED_BIN`、`HERDR_OPEN_CODE_BIN`、`HERDR_OPEN_HERDR_BIN`。编辑器覆盖变量设置在运行编辑器的一端。Zed URI 的空格、中文、`#`、`?` 等按 UTF-8 百分号编码。

## 验证

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

测试覆盖路径选择、URI 编码、参数传递、认证拒绝、消息大小及真实 TCP relay 请求；集成测试使用模拟编辑器，不需要桌面或 SSH 服务。CI 在 Linux/macOS 运行。实际 Herdr + SSH + 编辑器 GUI 需在目标机器进行联调。

设计参考：Herdr 官方插件接口、Zed 官方 SSH CLI，以及 [herdr-open-in-editor](https://github.com/timofey-TK/herdr-open-in-editor) 的 relay 思路；本项目为独立 Rust 实现。
