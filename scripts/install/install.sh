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

set -eu

REPO_URL="https://github.com/volt-hdl/volt"
# Empty here. The copy attached to a release names that release (release.yml
# writes it), so releases/download/vX.Y.Z/install.sh installs X.Y.Z.
PINNED_VERSION=""
BOOK_URL="https://volt-hdl.github.io/volt/tour/install.html"
MARKER="# added by the Volt installer"
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

# download URL FILE -> 0 on success, 2 when the server answered 404,
# 1 on any other failure.
download() {
    if command -v curl >/dev/null 2>&1; then
        code=$(curl -sSL --retry 3 -o "$2" -w '%{http_code}' "$1") || return 1
        case $code in
            200) return 0 ;;
            404) return 2 ;;
            *) warn "$1: HTTP $code"; return 1 ;;
        esac
    elif command -v wget >/dev/null 2>&1; then
        # wget exits 8 when the server sent an error response.
        if wget -q -O "$2" "$1"; then return 0; else code=$?; fi
        if [ "$code" -eq 8 ]; then return 2; fi
        return 1
    else
        die "neither curl nor wget is installed; install one of them and run this again"
    fi
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

no_release() {
    die "no published Volt release was found at $1.
Build from source instead (needs Rust, https://rustup.rs):
  git clone $REPO_URL
  cd volt
  cargo install --path crates/volt-driver
Details: $BOOK_URL#build-from-source"
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

path_line() {
    case $1 in
        */fish/config.fish) echo "set -gx PATH \"$2\" \$PATH $MARKER" ;;
        *) echo "export PATH=\"$2:\$PATH\" $MARKER" ;;
    esac
}

add_to_path() {
    profile=$(profile_file)
    if [ -f "$profile" ] && grep -qF "$MARKER" "$profile"; then
        say "PATH       already set in $profile"
        return
    fi
    mkdir -p "$(dirname "$profile")"
    line=$(path_line "$profile" "$1")
    # Start on a new line even if the file does not end with one.
    if [ -s "$profile" ] && [ -n "$(tail -c 1 "$profile")" ]; then
        printf '\n' >> "$profile"
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
        cat "$tmp_profile" > "$f"
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
            base="$REPO_URL/releases/download/v${version#v}"
        else
            base="$REPO_URL/releases/latest/download"
        fi
        name=$asset
        say "Downloading $base/$name"
        rc=0
        download "$base/SHA256SUMS" "$tmp/SHA256SUMS" || rc=$?
        [ $rc -eq 2 ] && no_release "$base"
        [ $rc -eq 0 ] || die "could not download $base/SHA256SUMS (network error?)"
        rc=0
        download "$base/$name" "$tmp/$name" || rc=$?
        [ $rc -eq 2 ] && die "the release has no archive $name"
        [ $rc -eq 0 ] || die "could not download $base/$name (network error?)"
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
