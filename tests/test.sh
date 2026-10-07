#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
export TEST_ROOT=$PWD TEST_DIR=$TMP HERDR_PLUGIN_CONFIG_DIR=$TMP/config
TEST_JQ_BIN=$(command -v jq)
export TEST_JQ_BIN
# Keep test POSIX paths intact when Git Bash invokes native Windows jq.
export MSYS_NO_PATHCONV=1
mkdir -p "$TMP/bin" "$HERDR_PLUGIN_CONFIG_DIR"

# Mock external commands; keep real jq parsing and FIFO transport.
cat > "$TMP/bin/editor" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$TEST_DIR/args"
SH

cat > "$TMP/bin/herdr" <<'SH'
#!/usr/bin/env bash
if [[ $1 == plugin ]]; then
    printf '%s\n' "$HERDR_PLUGIN_CONFIG_DIR"
    exit
fi
HERDR_PLUGIN_CONTEXT_JSON='{"workspace_cwd":"/remote/a b\\c"}' \
    bash "$TEST_ROOT/herdr-open" open code
for i in {1..100}; do
    [[ -f $TEST_DIR/args ]] && exit 0
    sleep 0.05
done
exit 1
SH

cat > "$TMP/bin/ssh" <<'SH'
#!/usr/bin/env bash
# Execute the observed remote command locally to exercise real FIFO transport.
for last; do :; done
exec bash -c "$last"
SH

cat > "$TMP/bin/jq" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
# Simulate Windows jq's CRLF output when decoding relay fields.
if [[ ${2:-} == *'.editor,.path'* ]]; then
    "$TEST_JQ_BIN" "$@" | awk '{sub(/\r$/, ""); printf "%s\r\n", $0}'
else
    exec "$TEST_JQ_BIN" "$@"
fi
SH

chmod +x "$TMP/bin/"*
export PATH=$TMP/bin:$PATH
export HERDR_OPEN_ZED_BIN=$TMP/bin/editor HERDR_OPEN_CODE_BIN=$TMP/bin/editor

# URI encoding and worktree path priority.
bash herdr-open remote zed nas '/repo/中文 #?'
[[ $(cat "$TMP/args") == 'ssh://nas/repo/%E4%B8%AD%E6%96%87%20%23%3F' ]]
HERDR_PLUGIN_CONTEXT_JSON='{"worktree":{"checkout_path":"/checkout"},"workspace_cwd":"/other"}' \
    bash herdr-open open zed
[[ $(cat "$TMP/args") == /checkout ]]

# Reject unsafe hosts and relative paths.
if bash herdr-open remote zed '-evil' /repo 2>/dev/null; then
    exit 1
fi
if bash herdr-open local code relative 2>/dev/null; then
    exit 1
fi

# Remote requests survive CRLF decoding; exiting removes the relay.
rm "$TMP/args"
bash herdr-open attach nas
[[ $(sed -n '1p' "$TMP/args") == --remote ]]
[[ $(sed -n '2p' "$TMP/args") == ssh-remote+nas ]]
[[ $(sed -n '3p' "$TMP/args") == '/remote/a b\c' ]]
[[ ! -d $HERDR_PLUGIN_CONFIG_DIR/relay ]]
printf 'Bash tests passed: paths, URI, validation, CRLF, FIFO relay, cleanup\n'
