# remote-code <target> <repo_dir>   本机 VS Code 打开远端 repo
# remote-zed  <target> <repo_dir>   本机 Zed 打开远端 repo
# ~ 会被 shell 展开；要远端家目录请写 '~'
# MSYS_NO_PATHCONV：Git Bash 不改写远端 POSIX 路径

uri_path() {
    MSYS_NO_PATHCONV=1 jq -rn --arg path "$1" \
        '$path | split("/") | map(@uri) | join("/")'
}

remote-zed() {
    local target=$1 repo_dir=$2 path
    path=$(uri_path "$repo_dir")
    [[ $path == /* ]] || path="/$path"
    MSYS_NO_PATHCONV=1 zed "ssh://$target$path"
}

remote-code() {
    local target=$1 repo_dir=$2
    MSYS_NO_PATHCONV=1 code --remote "ssh-remote+$target" "$repo_dir"
}
