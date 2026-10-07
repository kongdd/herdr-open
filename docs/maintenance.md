# 开发与维护

## 实现与限制

- 路径优先级：`worktree.checkout_path` → `workspace_cwd` → `focused_pane_cwd`。
- `herdr-open` 负责请求和打开编辑器；`herdr-open-relay` 仅监听请求，不启动 Herdr、切换 machine 或添加连接。每个 SSH 目标需要本机对应的 relay。
- 本机通过 SSH 接收远端用户私有 FIFO 中的 JSON，然后执行编辑器 CLI；无需反向端口、socat、Rust 或 Python。
- Zed 使用 `ssh://HOST/路径`，中文、空格等自动编码；VS Code 使用 `--remote ssh-remote+HOST`。远端无需桌面编辑器。
- SSH 的端口、密钥、跳板和 IPv6 地址由 `~/.ssh/config` 管理；helper 为非交互 SSH 补全 Homebrew、`~/.local/bin` 等常见安装目录。
- 每个远端用户仅允许一个 relay；不同 Herdr 会话共享它。不支持多客户端分流或 SSH 自动重连。
- 请求最多 512 字节，以保证 FIFO 写入原子性；超长路径明确报错。
- 远端 action 成功仅表示请求已发送；本机编辑器失败显示在 relay 的 stderr。
- 本机 helper 支持 Git Bash；插件宿主仅支持 Linux/macOS，本地 Windows workspace 尚不支持。

## 残留目录恢复

正常退出或 SSH 输入流关闭时删除 FIFO。强制杀进程或断电可能留下目录；确认旧 relay 已结束后，在远端清理：

```sh
rm -rf "$(herdr plugin config-dir kongdd.open)/relay"
```

## 手动诊断

```sh
./herdr-open local zed /absolute/path
./herdr-open remote code mac /absolute/path
```

本机可用 `HERDR_OPEN_ZED_BIN`、`HERDR_OPEN_CODE_BIN` 覆盖编辑器 CLI。

## 验证

```sh
bash -n herdr-open && bash -n herdr-open-relay
bash tests/test.sh
```

测试模拟编辑器、SSH、Herdr，使用真实 jq 和 FIFO，覆盖路径、URI、验证、断线与清理。CI 在 Linux/macOS 运行；真实 GUI 与服务器需要联调。
