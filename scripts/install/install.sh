#!/bin/sh
# Volt installer for Linux and macOS (ADR-0096).
#
#   curl -fsSL https://volt-hdl.github.io/volt/install.sh | sh
#
# Downloads the newest release archive for this platform, checks it against
# the release's SHA256SUMS, installs `volt` into ~/.volt/bin and adds that
# directory to PATH in the startup file of your shell (one line, marked
# "added by the Volt installer"). Running it again updates Volt. No root.
#
# Settings (environment variables; a piped script takes no arguments):
#   VOLT_VERSION=0.1.0     install this release instead of the newest one
#   VOLT_INSTALL_DIR=DIR   install into DIR/bin (default: ~/.volt)
#   VOLT_ARCHIVE=FILE      install from a local archive instead of
#                          downloading; checked against a SHA256SUMS file
#                          next to it when there is one
#   VOLT_UNINSTALL=1       remove the files and the PATH line again
#
# Example: curl -fsSL https://volt-hdl.github.io/volt/install.sh | VOLT_VERSION=0.1.0 sh
#
# Written for POSIX sh (dash, busybox, bash, zsh): no bash features.
# Before anything is downloaded the release is resolved: GitHub's API
# first, the releases/latest redirect when the API is rate limited. With no
# published release the script says so and stops. Transient network
# failures are tried again (3 attempts). This file must stay ASCII without
# a BOM (check-consistency, check 14).

set -eu

REPO_URL="https://github.com/volt-hdl/volt"
# Where releases are looked up and downloaded from. VOLT_INSTALL_TEST_SERVER
# exists for install.yml's fake server only (scripts/install/test/).
DOWNLOAD_REPO=$REPO_URL
API_REPO="https://api.github.com/repos/volt-hdl/volt"
if [ -n "${VOLT_INSTALL_TEST_SERVER:-}" ]; then
    DOWNLOAD_REPO="${VOLT_INSTALL_TEST_SERVER%/}/volt-hdl/volt"
    API_REPO="${VOLT_INSTALL_TEST_SERVER%/}/api/repos/volt-hdl/volt"
fi
# Transient network failures: this many attempts, waiting 1 s, then 2 s.
ATTEMPTS=3
# Empty here. The copy attached to a release names that release (release.yml
# writes it), so releases/download/vX.Y.Z/install.sh installs X.Y.Z.
PINNED_VERSION=""
BOOK_URL="https://volt-hdl.github.io/volt/tour/install.html"
SOURCE_ADVICE="Build from source instead (needs Rust, https://rustup.rs):
  git clone $REPO_URL
  cd volt
  cargo install --locked --path crates/volt-driver
More: $BOOK_URL#build-from-source"
NETWORK_ADVICE="This is usually a network problem (no connection, a proxy or a firewall) or a short GitHub outage.
Run the same command again in a few minutes, or install by hand: $BOOK_URL#manual-install"
MARKER="# added by the Volt installer"
# The same line, when the file did not end with a newline before it: the
# uninstall then takes that newline away again, so the file is restored
# byte for byte.
MARKER_NONL="$MARKER (file had no final newline)"
SUPPORTED="  Linux x86_64 (static binary, any distribution)
  macOS Apple silicon (arm64)
  macOS Intel (x86_64)"

say() { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

# The Rust target triple of the archive for this machine.
detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os/$arch" in
        Linux/x86_64 | Linux/amd64)
            echo x86_64-unknown-linux-musl ;;
        Darwin/arm64 | Darwin/aarch64)
            echo aarch64-apple-darwin ;;
        Darwin/x86_64)
            # A shell running under Rosetta reports x86_64 on Apple silicon;
            # the native arm64 binary is the better choice there.
            if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
                echo aarch64-apple-darwin
            else
                echo x86_64-apple-darwin
            fi ;;
        MINGW* | MSYS* | CYGWIN* | Windows_NT*)
            die "this is the Linux/macOS installer. On Windows, run in PowerShell:
  irm https://volt-hdl.github.io/volt/install.ps1 | iex" ;;
        *)
            die "no prebuilt Volt binary for $os $arch. Prebuilt binaries exist for:
$SUPPORTED
On other platforms, build from source (needs Rust): $BOOK_URL#build-from-source" ;;
    esac
}

# http_get URL FILE ACCEPT follow|noredirect: one GET into FILE. Sets
# HTTP_CODE (000: no complete HTTP answer), HTTP_LOCATION (the first
# redirect) and HTTP_ERROR. Returns 0 on a 2xx answer, and with noredirect
# on a 3xx one as well; 1 otherwise.
http_get() {
    HTTP_CODE=000
    HTTP_LOCATION=
    HTTP_ERROR=
    if command -v curl >/dev/null 2>&1; then
        http_get_curl "$@"
    elif command -v wget >/dev/null 2>&1; then
        http_get_wget "$@"
    else
        die "neither curl nor wget is installed; install one of them and run this again"
    fi
    case $HTTP_CODE in
        2??) return 0 ;;
        3??) [ "$4" = noredirect ] && return 0 ;;
    esac
    return 1
}

http_get_curl() {
    redirect=--no-location
    [ "$4" = follow ] && redirect=-L
    # A transfer that stalls (under 1 byte/s for 60 s) fails and is retried;
    # no overall time limit, so a slow line still finishes.
    if out=$(curl -sS "$redirect" -A volt-installer -H "Accept: $3" --connect-timeout 30 \
        --speed-limit 1 --speed-time 60 -o "$2" -w '%{http_code} %{redirect_url}' "$1" 2>"$tmp/http.err"); then
        HTTP_CODE=${out%% *}
        HTTP_LOCATION=${out#* }
    else
        # Also a transfer cut short (curl exit 18) after a 200: no complete
        # answer.
        HTTP_ERROR=$(sed 's/^curl: //' "$tmp/http.err" | head -n 1)
    fi
}

http_get_wget() {
    # -S prints every response's headers, redirects included: the first
    # Location is the redirect, the last status line the final answer.
    url=$1
    mode=$4
    set -- -S -O "$2" -U volt-installer --header "Accept: $3" -T 30
    if wget --version 2>/dev/null | grep -q 'GNU Wget'; then
        set -- "$@" --tries=1
        [ "$mode" = noredirect ] && set -- "$@" --max-redirect=0
    fi
    wget_rc=0
    wget "$@" "$url" 2>"$tmp/http.err" || wget_rc=$?
    # Header names may come in any case (busybox prints them as sent).
    HTTP_LOCATION=$(awk 'tolower($1) == "location:" { print $2; exit }' "$tmp/http.err" | tr -d '\r')
    if [ "$mode" = noredirect ]; then
        wget_code=$(awk '$1 ~ /^HTTP\// { print $2; exit }' "$tmp/http.err")
    else
        wget_code=$(awk '$1 ~ /^HTTP\// { c = $2 } END { print c }' "$tmp/http.err")
    fi
    # An error status (4xx, 5xx) is an answer, whatever the exit code (GNU
    # wget exits 8, busybox 1). A 2xx counts only when wget succeeded: a
    # download cut short after "200" leaves no complete answer.
    case $wget_code in
        [45]??) HTTP_CODE=$wget_code ;;
        [23]??)
            if [ "$wget_rc" -eq 0 ] || [ "$mode" = noredirect ]; then HTTP_CODE=$wget_code; fi ;;
    esac
    if [ "$HTTP_CODE" = 000 ]; then
        HTTP_ERROR="wget exit code $wget_rc: $(grep -v '^  ' "$tmp/http.err" | tail -n 1)"
    fi
}

# fetch URL FILE ACCEPT follow|noredirect: http_get, tried again on a
# transient failure (no answer, 408, 5xx). Waits 1 s, then 2 s.
fetch() {
    attempt=1
    while :; do
        if http_get "$@"; then return 0; fi
        case $HTTP_CODE in
            000 | 408 | 5??) ;;
            *) return 1 ;;
        esac
        [ "$HTTP_CODE" = 000 ] || HTTP_ERROR="HTTP $HTTP_CODE"
        [ "$attempt" -lt "$ATTEMPTS" ] || return 1
        say "           $HTTP_ERROR; trying again in $attempt s"
        sleep "$attempt"
        attempt=$((attempt + 1))
    done
}

# download URL FILE WHAT -> 0, or 2 when the server answered 404. Any other
# failure ends the install with a message naming WHAT.
download() {
    if fetch "$1" "$2" '*/*' follow; then return 0; fi
    [ "$HTTP_CODE" = 404 ] && return 2
    why=$HTTP_ERROR
    [ "$HTTP_CODE" = 000 ] || why="HTTP $HTTP_CODE"
    die "could not download $3 from $1
  $why
$NETWORK_ADVICE"
}

# Sets TAG to the newest published release (drafts and pre-releases do not
# count). Asks the GitHub API; when that is rate limited (403, 429) or
# fails, reads the redirect of releases/latest instead. No release at all
# ends the install with a notice.
resolve_latest() {
    if fetch "$API_REPO/releases/latest" "$tmp/latest.json" application/vnd.github+json follow; then
        TAG=$(grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' "$tmp/latest.json" | head -n 1 | sed 's/.*"\([^"]*\)"$/\1/')
        [ -n "$TAG" ] && return 0
        api_why="the answer named no release"
    else
        case $HTTP_CODE in
            404) no_release ;;
            403 | 429) api_why="rate limited (HTTP $HTTP_CODE)" ;;
            000) api_why=$HTTP_ERROR ;;
            *) api_why="HTTP $HTTP_CODE" ;;
        esac
    fi
    say "GitHub API: $api_why; asking $DOWNLOAD_REPO/releases/latest instead"
    if fetch "$DOWNLOAD_REPO/releases/latest" "$tmp/latest.html" '*/*' noredirect; then
        case $HTTP_LOCATION in
            */releases/tag/*)
                TAG=${HTTP_LOCATION##*/releases/tag/}
                return 0 ;;
            */releases | */releases/) no_release ;;
        esac
        web_why="HTTP $HTTP_CODE, redirect to '$HTTP_LOCATION'"
    elif [ "$HTTP_CODE" = 000 ]; then
        web_why=$HTTP_ERROR
    else
        web_why="HTTP $HTTP_CODE"
    fi
    die "could not find out which Volt release is the newest.
  $API_REPO/releases/latest: $api_why
  $DOWNLOAD_REPO/releases/latest: $web_why
$NETWORK_ADVICE
Or name the release: VOLT_VERSION=0.1.0 (the releases: $REPO_URL/releases)"
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        die "neither sha256sum nor shasum is installed, so the download cannot be verified"
    fi
}

# verify ARCHIVE SUMS NAME: the line for NAME in SUMS must carry the hash of
# ARCHIVE. Lines look like "<hash>  <name>" or "<hash> *<name>".
verify() {
    expected=$(awk -v n="$3" '$2 == n || $2 == "*" n { print $1; exit }' "$2")
    [ -n "$expected" ] || die "SHA256SUMS has no line for $3; nothing was installed"
    actual=$(sha256_of "$1")
    if [ "$actual" != "$expected" ]; then
        die "checksum mismatch for $3
  expected $expected
  got      $actual
The download is damaged or was altered. Nothing was installed."
    fi
    say "Verified   SHA256 $actual"
}

# Not an error (no "error:" prefix): there is nothing to install yet.
no_release() {
    printf 'No Volt release has been published yet.\n%s\n' "$SOURCE_ADVICE" >&2
    exit 1
}

# The startup file that puts the install directory on PATH, by login shell.
profile_file() {
    case $(basename "${SHELL:-sh}") in
        zsh) echo "${ZDOTDIR:-$HOME}/.zshrc" ;;
        bash)
            # macOS terminals start login shells, which read .bash_profile;
            # Linux terminals start interactive shells, which read .bashrc.
            if [ "$(uname -s)" = Darwin ]; then
                echo "$HOME/.bash_profile"
            else
                echo "$HOME/.bashrc"
            fi ;;
        fish) echo "${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish" ;;
        *) echo "$HOME/.profile" ;;
    esac
}

# path_line FILE DIR MARKER
path_line() {
    case $1 in
        */fish/config.fish) echo "set -gx PATH \"$2\" \$PATH $3" ;;
        *) echo "export PATH=\"$2:\$PATH\" $3" ;;
    esac
}

add_to_path() {
    profile=$(profile_file)
    if [ -f "$profile" ] && grep -qF "$MARKER" "$profile"; then
        say "PATH       already set in $profile"
        return
    fi
    mkdir -p "$(dirname "$profile")"
    # Start on a new line even if the file does not end with one.
    if [ -s "$profile" ] && [ -n "$(tail -c 1 "$profile")" ]; then
        printf '\n' >> "$profile"
        line=$(path_line "$profile" "$1" "$MARKER_NONL")
    else
        line=$(path_line "$profile" "$1" "$MARKER")
    fi
    printf '%s\n' "$line" >> "$profile"
    say "PATH       added to $profile:"
    say "             $line"
}

# Remove every marked line from the startup files this script may have
# written. A file left empty is removed: it held nothing but our line.
remove_from_path() {
    for f in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.bash_profile" \
        "${ZDOTDIR:-$HOME}/.zshrc" "${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish"; do
        if [ ! -f "$f" ] || ! grep -qF "$MARKER" "$f"; then continue; fi
        tmp_profile="$f.volt-uninstall.$$"
        grep -vF "$MARKER" "$f" > "$tmp_profile" || true
        # cat keeps the file's owner, mode and any symlink in place.
        if tail -n 1 "$f" | grep -qF "$MARKER_NONL"; then
            # Our line was last and the newline before it was ours too.
            printf '%s' "$(cat "$tmp_profile")" > "$f"
        else
            cat "$tmp_profile" > "$f"
        fi
        rm -f "$tmp_profile"
        if [ ! -s "$f" ]; then
            rm -f "$f"
            say "Removed    $f (it held nothing but the PATH line)"
        else
            say "Removed    the PATH line from $f"
        fi
    done
}

uninstall() {
    dir=$1
    for f in bin/volt LICENSE-APACHE LICENSE-MIT README.md; do
        if [ -e "$dir/$f" ]; then
            rm -f "$dir/$f"
            say "Removed    $dir/$f"
        fi
    done
    rmdir "$dir/bin" 2>/dev/null || true
    rmdir "$dir" 2>/dev/null || true
    remove_from_path
    say "Volt is uninstalled. Terminals that are already open keep the old PATH until they are closed."
}

main() {
    dir=${VOLT_INSTALL_DIR:-$HOME/.volt}
    case $dir in
        /*) ;;
        *) dir="$(pwd)/$dir" ;;
    esac
    bin="$dir/bin"

    if [ "${VOLT_UNINSTALL:-}" = 1 ]; then
        uninstall "$dir"
        return
    fi

    target=$(detect_target)
    asset="volt-$target.tar.gz"
    tmp=$(mktemp -d 2>/dev/null || mktemp -d -t volt-install)
    trap 'rm -rf "$tmp"' EXIT
    trap 'exit 130' INT TERM

    if [ -n "${VOLT_ARCHIVE:-}" ]; then
        [ -f "$VOLT_ARCHIVE" ] || die "VOLT_ARCHIVE: no such file: $VOLT_ARCHIVE"
        name=$(basename "$VOLT_ARCHIVE")
        cp "$VOLT_ARCHIVE" "$tmp/$name"
        sums="$(dirname "$VOLT_ARCHIVE")/SHA256SUMS"
        say "Installing Volt from $VOLT_ARCHIVE"
        if [ -f "$sums" ]; then
            verify "$tmp/$name" "$sums" "$name"
        else
            warn "no SHA256SUMS next to $name: the archive is not verified"
        fi
    else
        version=${VOLT_VERSION:-$PINNED_VERSION}
        if [ -n "$version" ]; then
            TAG="v${version#v}"
        else
            resolve_latest
            say "Newest release: $TAG"
        fi
        base="$DOWNLOAD_REPO/releases/download/$TAG"
        name=$asset
        say "Downloading $base/$name"
        rc=0
        download "$base/SHA256SUMS" "$tmp/SHA256SUMS" "SHA256SUMS of Volt $TAG" || rc=$?
        if [ $rc -eq 2 ]; then
            printf 'Volt release %s was not found. The published releases are listed at\n%s\n%s\n' \
                "$TAG" "$REPO_URL/releases" "$SOURCE_ADVICE" >&2
            exit 1
        fi
        rc=0
        download "$base/$name" "$tmp/$name" "the Volt $TAG archive" || rc=$?
        [ $rc -eq 2 ] && die "Volt release $TAG has no archive $name.
Install by hand ($BOOK_URL#manual-install) or build from source ($BOOK_URL#build-from-source)."
        verify "$tmp/$name" "$tmp/SHA256SUMS" "$name"
    fi

    mkdir "$tmp/unpack"
    tar -xzf "$tmp/$name" -C "$tmp/unpack" || die "cannot unpack $name; nothing was installed"
    # The archive holds one folder (volt-<target>/) with the binary in it;
    # a binary at the top is accepted as well.
    src=
    for candidate in "$tmp/unpack/volt" "$tmp"/unpack/*/volt; do
        if [ -f "$candidate" ]; then
            src=$candidate
            break
        fi
    done
    [ -n "$src" ] || die "no volt binary in $name; nothing was installed"
    srcdir=$(dirname "$src")

    old=
    if [ -x "$bin/volt" ]; then
        old=$("$bin/volt" --version 2>/dev/null || echo "an unknown version")
    fi
    mkdir -p "$bin"
    # Copy, then rename over the old binary: a running volt keeps working.
    cp "$src" "$bin/volt.new.$$"
    chmod 755 "$bin/volt.new.$$"
    mv -f "$bin/volt.new.$$" "$bin/volt"
    for f in LICENSE-APACHE LICENSE-MIT README.md; do
        if [ -f "$srcdir/$f" ]; then cp "$srcdir/$f" "$dir/$f"; fi
    done
    if [ "$(uname -s)" = Darwin ]; then
        xattr -d com.apple.quarantine "$bin/volt" 2>/dev/null || true
    fi

    new=$("$bin/volt" --version) || die "$bin/volt does not run"
    if [ -n "$old" ] && [ "$old" != "$new" ]; then
        say "Updated    $old -> $new in $bin"
    elif [ -n "$old" ]; then
        say "Reinstalled $new in $bin"
    else
        say "Installed  $new in $bin"
    fi

    add_to_path "$bin"
    found=$(command -v volt 2>/dev/null || true)
    if [ -n "$found" ] && [ "$found" != "$bin/volt" ]; then
        warn "another volt comes earlier on PATH: $found"
    fi

    say ""
    say "$new is ready. Open a new terminal (or run: export PATH=\"$bin:\$PATH\"), then:"
    say "  volt doctor      # which commands work here"
    say "  volt new blinky  # a project to start from"
    say "Update: run the same install command again. Uninstall: VOLT_UNINSTALL=1 with it."
}

# Everything above only defines functions: a download cut short runs nothing.
main "$@"
