#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

TMP=$(mktemp -d)
relay_pid=''
cleanup() {
    if [[ -n $relay_pid ]]; then
        kill "$relay_pid" 2>/dev/null || :
        wait "$relay_pid" 2>/dev/null || :
    fi
    rm -rf "$TMP"
}
trap cleanup EXIT
export TEST_ROOT=$PWD TEST_DIR=$TMP HERDR_PLUGIN_CONFIG_DIR=$TMP/config
TEST_JQ_BIN=$(command -v jq)
export TEST_JQ_BIN
# Keep test POSIX paths intact when Git Bash invokes native Windows jq.
export MSYS_NO_PATHCONV=1
# Local-path checks must also work when the test suite itself runs over SSH.
unset SSH_CONNECTION SSH_CLIENT SSH_TTY
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
printf 'started\n' > "$TEST_DIR/herdr-ui"
exit 1
SH

cat > "$TMP/bin/ssh" <<'SH'
#!/usr/bin/env bash
# Execute the observed remote command locally to exercise real FIFO transport.
if [[ ${TEST_SSH_DROP:-0} == 1 ]]; then
    printf '{"ready":true}\n'
    sleep 0.3
    exit 42
fi
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
bash herdr-open remote zed mac '/repo/中文 #?'
[[ $(cat "$TMP/args") == 'ssh://mac/repo/%E4%B8%AD%E6%96%87%20%23%3F' ]]
HERDR_PLUGIN_CONTEXT_JSON='{"worktree":{"checkout_path":"/checkout"},"workspace_cwd":"/other"}' \
    bash herdr-open open zed
[[ $(cat "$TMP/args") == /checkout ]]

# Reject unsafe hosts and relative paths.
if bash herdr-open remote zed '-evil' /repo 2>/dev/null; then
    exit 1
fi
if bash herdr-open-relay '-evil' 2>/dev/null; then
    exit 1
fi
if bash herdr-open local code relative 2>/dev/null; then
    exit 1
fi
if bash herdr-open local code $'/repo/\nunsafe' 2>/dev/null; then
    exit 1
fi
if bash herdr-open-relay mac extra 2>/dev/null; then
    exit 1
fi

# Relay does not launch Herdr; both remote entry points can share it.
rm "$TMP/args"
(cd "$TMP"; exec bash "$TEST_ROOT/herdr-open-relay" mac) 2> "$TMP/relay.log" &
relay_pid=$!
for i in {1..100}; do
    grep -q 'relay ready' "$TMP/relay.log" && break
    sleep 0.05
done
grep -q 'relay ready' "$TMP/relay.log"
[[ ! -f $TMP/herdr-ui ]]

wait_editor() {
    for i in {1..100}; do
        [[ -f $TMP/args ]] && return
        sleep 0.05
    done
    return 1
}

# Machine-mode servers need not inherit SSH environment variables.
HERDR_PLUGIN_CONTEXT_JSON='{"workspace_cwd":"/remote/中文"}' \
    bash herdr-open open zed
wait_editor
[[ $(cat "$TMP/args") == 'ssh://mac/remote/%E4%B8%AD%E6%96%87' ]]

# An invalid request must not stop the relay.
printf '%s\n' '{"editor":"bad","path":"/repo"}' > "$HERDR_PLUGIN_CONFIG_DIR/relay/requests"
rm "$TMP/args"
SSH_CONNECTION='127.0.0.1 1234 127.0.0.2 22' \
    HERDR_PLUGIN_CONTEXT_JSON='{"focused_pane_cwd":"/remote/ssh home"}' \
    bash herdr-open open code
wait_editor
[[ $(sed -n '2p' "$TMP/args") == ssh-remote+mac ]]
[[ $(sed -n '3p' "$TMP/args") == '/remote/ssh home' ]]

# A duplicate must not remove the first relay; stopping an idle relay cleans up.
if bash herdr-open-relay mac 2> "$TMP/duplicate.log"; then
    exit 1
fi
[[ -p $HERDR_PLUGIN_CONFIG_DIR/relay/requests ]]
kill "$relay_pid"
status=0
wait "$relay_pid" || status=$?
relay_pid=''
[[ $status == 143 && ! -d $HERDR_PLUGIN_CONFIG_DIR/relay ]]

# A disconnected standalone relay reports failure instead of silently succeeding.
if TEST_SSH_DROP=1 bash herdr-open-relay mac 2> "$TMP/drop.log"; then
    exit 1
fi
grep -q 'SSH relay disconnected' "$TMP/drop.log"

# A stale FIFO cannot block an action indefinitely.
mkdir "$HERDR_PLUGIN_CONFIG_DIR/relay"
mkfifo "$HERDR_PLUGIN_CONFIG_DIR/relay/requests"
if HERDR_PLUGIN_CONTEXT_JSON='{"workspace_cwd":"/repo"}' \
    bash herdr-open open zed 2> "$TMP/stale.log"; then
    exit 1
fi
grep -q 'relay unavailable' "$TMP/stale.log"
printf 'Bash tests passed: relay, paths, validation, disconnect, cleanup\n'
