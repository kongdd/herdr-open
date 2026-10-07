# herdr-open

用本机 Zed / VS Code 打开 Herdr 当前目录。

## 安装

本机和 mac 需要 Herdr ≥ 0.9.2、Bash、jq。本机需安装编辑器 CLI；VS Code 需 Remote - SSH 扩展。Windows 本机使用 Git Bash。

在本机和 mac 执行：

```sh
git clone https://github.com/kongdd/herdr-open
cd herdr-open
herdr plugin link "$PWD"
```

## 使用

在本机仓库目录运行，并保持该终端打开：

```sh
./herdr-open-relay mac
```

之后照常通过 machine 或 `ssh mac` 使用 Herdr。

以下快捷键配置：machine 模式放在本机，SSH 登录模式放在 mac。

```toml
[[keys.command]]
key = "prefix+o"
type = "plugin_action"
command = "kongdd.open.open-zed"

[[keys.command]]
key = "prefix+v"
type = "plugin_action"
command = "kongdd.open.open-code"
```

默认 `Ctrl+B` → `O` 打开 Zed，`Ctrl+B` → `V` 打开 VS Code；`Ctrl+C` 停止 relay。
