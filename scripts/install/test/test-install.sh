#!/bin/sh
# End-to-end test of install.sh (install.yml; ADR-0096). Changes $HOME:
# run it on a CI runner or in a throw-away account, not on your machine.
#
#   scripts/install/test/test-install.sh <archive.tar.gz> <shell>...
#
# For each shell (bash, zsh, sh): install from the archive through a pipe
# into sh, as `curl ... | sh` does; start a NEW shell with a bare PATH and
# check that its startup file finds volt; install again (the update path)
# and check that the PATH line is not repeated; uninstall and check that
# the files are gone and the startup file is byte for byte what it was.
# Then: a wrong SHA256SUMS stops the install and leaves nothing behind, and
# a release that does not exist ends with the build-from-source advice.
set -eu

[ $# -ge 2 ] || { echo "usage: $0 <archive.tar.gz> <shell>..." >&2; exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
installer="$here/../install.sh"
archive=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
shift
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fails=0

fail() {
    echo "FAIL: $*"
    fails=$((fails + 1))
}

# The archive with a correct SHA256SUMS next to it, as a release has it.
mkdir "$work/good"
cp "$archive" "$work/good/"
name=$(basename "$archive")
(cd "$work/good" && { sha256sum "$name" 2>/dev/null || shasum -a 256 "$name"; } > SHA256SUMS)
cat "$work/good/SHA256SUMS"

# run_installer VAR=value... : like `curl -fsSL .../install.sh | sh`, with
# the settings passed through env. Not as `VAR=value run_installer`: in
# bash's POSIX mode (macOS /bin/sh) an assignment before a function call
# stays set in the calling shell afterwards.
run_installer() {
    env "$@" sh < "$installer"
}

profile_of() {
    case $1 in
        zsh) echo "$HOME/.zshrc" ;;
        bash) if [ "$(uname -s)" = Darwin ]; then echo "$HOME/.bash_profile"; else echo "$HOME/.bashrc"; fi ;;
        *) echo "$HOME/.profile" ;;
    esac
}

# A new terminal: an empty environment, a bare PATH, the shell's own
# startup files.
new_shell() {
    sh_name=$1
    cmd=$2
    case $sh_name in
        zsh) flags=-ic ;;
        bash) if [ "$(uname -s)" = Darwin ]; then flags=-lc; else flags=-ic; fi ;;
        *) flags=-lc ;;
    esac
    env -i HOME="$HOME" USER="${USER:-runner}" TERM=dumb PATH=/usr/bin:/bin "$sh_name" "$flags" "$cmd" 2>/dev/null
}

for shell_name in "$@"; do
    echo "=== $shell_name ==="
    shell_path=$(command -v "$shell_name") || { fail "$shell_name is not installed"; continue; }
    profile=$(profile_of "$shell_name")
    had_profile=no
    if [ -f "$profile" ]; then
        had_profile=yes
        cp "$profile" "$work/profile.before"
    fi

    echo "--- install"
    run_installer SHELL="$shell_path" VOLT_ARCHIVE="$work/good/$name" || fail "$shell_name: install exited $?"
    [ -x "$HOME/.volt/bin/volt" ] || fail "$shell_name: ~/.volt/bin/volt missing"

    echo "--- new $shell_name: volt --version"
    out=$(new_shell "$shell_name" 'command -v volt && volt --version') || fail "$shell_name: new shell cannot run volt"
    echo "$out"
    case $out in
        "$HOME/.volt/bin/volt"*"volt "*) ;;
        *) fail "$shell_name: new shell did not find ~/.volt/bin/volt" ;;
    esac

    echo "--- install again (update path)"
    run_installer SHELL="$shell_path" VOLT_ARCHIVE="$work/good/$name" || fail "$shell_name: second install exited $?"
    count=$(grep -cF "added by the Volt installer" "$profile" || true)
    [ "$count" = 1 ] || fail "$shell_name: $count PATH lines in $profile after two installs, expected 1"
    new_shell "$shell_name" 'volt --version' || fail "$shell_name: volt missing after the update"

    echo "--- uninstall"
    run_installer SHELL="$shell_path" VOLT_UNINSTALL=1 || fail "$shell_name: uninstall exited $?"
    [ ! -e "$HOME/.volt" ] || fail "$shell_name: ~/.volt still exists: $(ls -R "$HOME/.volt")"
    if [ "$had_profile" = yes ]; then
        cmp "$work/profile.before" "$profile" || fail "$shell_name: $profile differs from before the install"
    else
        [ ! -e "$profile" ] || fail "$shell_name: $profile was created and not removed"
    fi
    if new_shell "$shell_name" 'command -v volt' >/dev/null; then
        fail "$shell_name: a new shell still finds volt after the uninstall"
    fi
done

echo "=== wrong checksum ==="
mkdir "$work/bad"
cp "$archive" "$work/bad/"
echo "0000000000000000000000000000000000000000000000000000000000000000  $name" > "$work/bad/SHA256SUMS"
snapshot() { for f in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.zshrc"; do [ -f "$f" ] && cksum "$f"; done; true; }
before=$(snapshot)
if run_installer VOLT_INSTALL_DIR="$work/bad-install" VOLT_ARCHIVE="$work/bad/$name"; then
    fail "install with a wrong SHA256SUMS succeeded"
fi
[ ! -e "$work/bad-install" ] || fail "wrong checksum left files: $(ls -R "$work/bad-install")"
[ "$before" = "$(snapshot)" ] || fail "wrong checksum changed a startup file"

echo "=== release that does not exist ==="
out=$(run_installer VOLT_VERSION=0.0.0 VOLT_INSTALL_DIR="$work/none" 2>&1) && fail "install of v0.0.0 succeeded"
echo "$out"
case $out in
    *"Volt release v0.0.0 was not found"*"cargo install"*) ;;
    *) fail "no build-from-source advice for a missing release" ;;
esac
[ ! -e "$work/none" ] || fail "missing release left files"

if [ "$fails" -ne 0 ]; then
    echo "$fails check(s) failed"
    exit 1
fi
echo "all install checks passed"
