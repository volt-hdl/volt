#!/bin/sh
# Package one volt binary as a release archive (ADR-0093, ADR-0096).
#
#   scripts/release/package.sh <target> <binary> <version> [out-dir]
#
# Writes <out-dir>/volt-<target>.zip (Windows targets) or .tar.gz (others);
# out-dir defaults to dist. The archive name carries no version, so
# https://github.com/volt-hdl/volt/releases/latest/download/<name> always
# points at the newest release. It unpacks into one folder named like the
# archive (volt-<target>/) holding the binary, both licenses and a README;
# the install scripts (scripts/install/) expect exactly that layout.
#
# release.yml and install.yml both call this script, so the archive the
# install tests use is built the same way as the one a release ships.
# Prints the archive path on the last line of stdout.
set -eu

if [ $# -lt 3 ]; then
    echo "usage: $0 <target> <binary> <version> [out-dir]" >&2
    exit 2
fi
target=$1
binary=$2
version=$3
out=${4:-dist}

case $target in
    *-windows-*) exe=.exe; kind=zip ;;
    *) exe=; kind=tar.gz ;;
esac

root=$(cd "$(dirname "$0")/../.." && pwd)
name="volt-$target"
mkdir -p "$out"
out=$(cd "$out" && pwd)
rm -rf "${out:?}/$name" "$out/$name.$kind"
mkdir "$out/$name"
cp "$binary" "$out/$name/volt$exe"
chmod 755 "$out/$name/volt$exe"
cp "$root/LICENSE-APACHE" "$root/LICENSE-MIT" "$out/$name/"
cat > "$out/$name/README.md" <<EOF
# Volt HDL $version ($target)

Volt is a hardware description language that emits SystemVerilog and
checks clock-domain crossings in its type system.

Put \`volt$exe\` on your PATH (or use the install script, below), then:

    volt doctor             # which commands work here, what to install
    volt new blinky         # a project with a counter, a test, contracts
    cd blinky
    volt check counter.volt
    volt build counter.volt # SystemVerilog in build/rtl/

\`volt build\`, \`check\`, \`explain\`, \`new\` and \`lsp\` need nothing else.
\`volt test\`/\`run\` need Verilator, \`volt verify\` needs SymbiYosys
(or Docker for both).

Install script (downloads, verifies SHA256SUMS, sets PATH):

    Windows:       irm https://volt-hdl.github.io/volt/install.ps1 | iex
    Linux, macOS:  curl -fsSL https://volt-hdl.github.io/volt/install.sh | sh

The binary is not code-signed. Windows SmartScreen: "More info" ->
"Run anyway". macOS: \`xattr -d com.apple.quarantine volt\`.
Checksums and build provenance: see the release page.

Documentation: https://volt-hdl.github.io/volt/
License: MIT OR Apache-2.0 (LICENSE-MIT, LICENSE-APACHE).
EOF

cd "$out"
if [ "$kind" = zip ]; then
    if command -v 7z >/dev/null 2>&1; then
        7z a -tzip -bso0 "$name.zip" "$name"
    else
        zip -qr "$name.zip" "$name"
    fi
else
    tar -czf "$name.tar.gz" "$name"
fi
rm -rf "${out:?}/$name"
echo "$out/$name.$kind"
