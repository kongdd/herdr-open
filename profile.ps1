# remote-code <target> <repo_dir>   本机 VS Code 打开远端 repo
# remote-zed  <target> <repo_dir>   本机 Zed 打开远端 repo

function uriPath {
    param([string]$Path)
    ($Path -split '/' | ForEach-Object { [uri]::EscapeDataString($_) }) -join '/'
}

function remote-zed {
    param([string]$Target, [string]$RepoDir)
    $path = uriPath -Path $RepoDir
    if ($path -notlike '/*') { $path = "/$path" }
    zed "ssh://$Target$path"
}

function remote-code {
    param([string]$Target, [string]$RepoDir)
    code --remote "ssh-remote+$Target" $RepoDir
}
