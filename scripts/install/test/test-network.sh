#!/bin/sh
# Network failure tests of install.sh against a fake GitHub
# (fake-github.py; install.yml; ADR-0096, appendix).
#
#   scripts/install/test/test-network.sh <archive.tar.gz>
#
# Each scenario pipes install.sh into sh, the way the one-line command
# does, with VOLT_INSTALL_TEST_SERVER pointing at the fake server,
# VOLT_INSTALL_DIR at a temporary folder and HOME at a temporary home (the
# startup file a successful install writes lands there). Checks: the
# expected message, no shell error, exit code 1 on a failure, nothing left
# in the install folder; two scenarios succeed (rate-limited API, transient
# errors) and are uninstalled again. The real ~/.volt must not change.
set -eu

[ $# -eq 1 ] || { echo "usage: $0 <archive.tar.gz>" >&2; exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
installer="$here/../install.sh"
archive=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
work=$(mktemp -d)
python=$(command -v python3 || command -v python)
fails=0
server=
cleanup() {
    # Cleanup trap: the server may already be gone; a failed kill must not replace the test's exit code.
    [ -z "$server" ] || kill "$server" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*"
    fails=$((fails + 1))
}

# The real install folder, as it is now: it must look the same at the end.
default_dir_state() {
    if [ -e "$HOME/.volt" ]; then ls -lR "$HOME/.volt"; else echo absent; fi
}
default_before=$(default_dir_state)

"$python" --version
"$python" -u "$here/fake-github.py" "$archive" "$work/port" 2>"$work/server.log" &
server=$!
i=0
while [ ! -s "$work/port" ] && [ $i -lt 600 ]; do
    sleep 0.1
    i=$((i + 1))
done
if [ ! -s "$work/port" ]; then
    echo "the fake server did not start within 60 s ($python, pid $server)"
    ps -p "$server" -o pid=,stat=,etime=,command= || echo "the server process is gone"
    ls -la "$work"
    cat "$work/server.log"
    exit 1
fi
port=$(cat "$work/port")
if command -v curl >/dev/null 2>&1; then
    fetcher=curl
elif wget --version 2>/dev/null | grep -q 'GNU Wget'; then
    fetcher="GNU wget"
else
    fetcher="wget (not GNU, e.g. busybox)"
fi
echo "fake GitHub on 127.0.0.1:$port; the installer will use $fetcher"
run=$(od -An -N4 -tx4 /dev/urandom | tr -d ' ')
serial=0
mkdir "$work/home"

# run_installer SCENARIO VAR=value...: sets OUT and CODE.
run_installer() {
    serial=$((serial + 1))
    scenario=$1
    shift
    CODE=0
    OUT=$(env HOME="$work/home" SHELL=/bin/sh \
        VOLT_INSTALL_TEST_SERVER="http://127.0.0.1:$port/$scenario-$run$serial" \
        "$@" sh < "$installer" 2>&1) || CODE=$?
    echo "--- $scenario: exit $CODE"
    echo "$OUT"
}

# check_clean LABEL: no shell error message in OUT.
check_clean() {
    if printf '%s\n' "$OUT" | grep -Eq '(^|[^a-z])(sh|bash|dash|zsh): |line [0-9]+:|unbound variable|syntax error'; then
        fail "$1: a shell error in the output"
    fi
}

# expect LABEL TEXT...: every TEXT is in OUT.
expect() {
    label=$1
    shift
    for t in "$@"; do
        case $OUT in
            *"$t"*) ;;
            *) fail "$label: output lacks '$t'" ;;
        esac
    done
}

# failure SCENARIO TEXT...: exit code 1, the texts, no install folder.
failure() {
    s=$1
    shift
    echo "=== $s"
    run_installer "$s" VOLT_INSTALL_DIR="$work/$s"
    [ "$CODE" -eq 1 ] || fail "$s: exit code $CODE, expected 1"
    check_clean "$s"
    expect "$s" "$@"
    [ ! -e "$work/$s" ] || fail "$s: left $work/$s behind"
}

# success SCENARIO TEXT...: installs, then uninstalls again.
success() {
    s=$1
    shift
    echo "=== $s"
    run_installer "$s" VOLT_INSTALL_DIR="$work/$s"
    [ "$CODE" -eq 0 ] || fail "$s: exit code $CODE, expected 0"
    check_clean "$s"
    expect "$s" "$@"
    [ -x "$work/$s/bin/volt" ] || fail "$s: volt was not installed"
    run_installer "$s" VOLT_INSTALL_DIR="$work/$s" VOLT_UNINSTALL=1
    [ "$CODE" -eq 0 ] || fail "$s uninstall: exit code $CODE"
    [ ! -e "$work/$s" ] || fail "$s: uninstall left $work/$s behind"
}

none='No Volt release has been published yet.'
source='cargo install --locked --path crates/volt-driver'
book='https://volt-hdl.github.io/volt/tour/install.html#build-from-source'
retry='trying again in 1 s'

failure none "$none" "$source" "$book"
case $OUT in *error:*) fail "none: the notice is printed as an error" ;; esac
failure ratelimit-none 'rate limited (HTTP 429)' "$none" "$source"
failure asset404 'Newest release: v9.9.9' 'error: Volt release v9.9.9 has no archive' '#manual-install'
failure cut 'error: could not download the Volt v9.9.9 archive' "$retry" 'trying again in 2 s' 'network problem' '#manual-install'
failure down 'error: could not find out which Volt release is the newest' "$retry" 'network problem'
success ratelimit 'rate limited (HTTP 403)' 'Newest release: v9.9.9' 'Verified' 'Installed'
success flaky "$retry" 'Newest release: v9.9.9' 'HTTP 503' 'Verified' 'Installed'

echo "=== VOLT_VERSION names a release that does not exist"
run_installer none VOLT_INSTALL_DIR="$work/pinned" VOLT_VERSION=0.0.1
[ "$CODE" -eq 1 ] || fail "pinned: exit code $CODE, expected 1"
check_clean pinned
expect pinned 'Volt release v0.0.1 was not found' "$source"
case $OUT in *error:* | *"trying again"*) fail "pinned: a 404 was reported as an error or retried" ;; esac
[ ! -e "$work/pinned" ] || fail "pinned: left $work/pinned behind"

[ "$(default_dir_state)" = "$default_before" ] || fail "the default install folder ~/.volt changed"
[ ! -s "$work/home/.profile" ] || fail "a PATH line is left in the temporary ~/.profile: $(cat "$work/home/.profile")"

if [ "$fails" -ne 0 ]; then
    echo "$fails check(s) failed"
    exit 1
fi
echo "all network checks passed"
