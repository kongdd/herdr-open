# herdr-open

用本机 Zed / VS Code 打开 Herdr 当前 workspace，支持本地和 SSH 远程。一个 Bash 脚本，依赖 Bash、jq、OpenSSH，无编译、监听端口或后台服务。

## 安装

macOS：`brew install jq`；Debian/Ubuntu：`sudo apt install jq`。
本机、服务器都需要 Bash、jq 和 Herdr ≥ 0.9.2。本机需要 `zed` / `code` CLI，VS Code 需要 Remote - SSH 扩展。

```sh
herdr plugin install kongdd/herdr-open
```

本机 helper 可直接使用插件目录里的脚本，或克隆后加入 PATH：

```sh
git clone https://github.com/kongdd/herdr-open
mkdir -p ~/.local/bin
ln -s "$PWD/herdr-open/herdr-open" ~/.local/bin/herdr-open
```

开发时 `herdr plugin link "$PWD"`，无需 build。

## 快捷键

Herdr 配置中添加；远程模式放在服务器端配置：

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

路径：`worktree.checkout_path` → `workspace_cwd` → `focused_pane_cwd`。

## SSH 远程

在服务器安装相同插件，确认 `ssh nas herdr --version` 能运行，然后在本机：

```sh
herdr-open attach nas
herdr-open attach nas -- --session work
```

按快捷键后，本机执行 `zed ssh://nas/远端路径` 或 `code --remote ssh-remote+nas 远端路径`。中文、空格等在 Zed URI 中自动编码。服务器不用安装桌面编辑器。

SSH 别名的端口、密钥、跳板和 IPv6 地址放在 `~/.ssh/config`。helper 建立 SSH 流，远端插件将 JSON 请求写入用户私有 FIFO，SSH 传回本机后启动编辑器。无需反向端口、socat、Rust 或 Python。

每个服务器用户只允许一个 attach；FIFO 在 Herdr 插件配置目录下，权限仅当前用户可访问。退出时关闭 SSH、读取进程并删除 FIFO。强制杀进程或断电可能留下目录，确认旧 attach 已结束后在服务器清理：

```sh
rm -rf "$(herdr plugin config-dir kongdd.open)/relay"
```

普通 `herdr --remote nas` 不会创建该 SSH 流，需要使用 helper。远程请求最多 512 字节（保证 FIFO 写入原子性），超长路径明确报错。远端 action 成功表示请求已发送；编辑器失败在本机 stderr 显示。第一版不支持 Windows helper、多客户端共享同一远端用户或 SSH 流自动重连。

## 手动打开

```sh
herdr-open local zed /absolute/path
herdr-open remote code nas /absolute/path
```

覆盖 CLI：`HERDR_OPEN_ZED_BIN`、`HERDR_OPEN_CODE_BIN`、`HERDR_OPEN_HERDR_BIN`，在执行编辑器的本机设置。

## 验证

```sh
bash -n herdr-open
bash tests/test.sh
```

测试使用模拟编辑器、SSH、Herdr，验证路径、URI、FIFO 传输与退出清理。CI 在 Linux/macOS 运行；真实编辑器 GUI 和服务器需在目标机器联调。
