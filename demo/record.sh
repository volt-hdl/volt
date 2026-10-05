#!/usr/bin/env bash
# Records the README demo, demo/cdc-demo.gif, from demo/cdc.tape and
# prints the same flow as plain text for the README.
#
#   bash demo/record.sh
#
# Needs git, bash (Git Bash on Windows) and Docker. The volt binary is
# built from `git archive HEAD`: commit a compiler change before recording
# it. One container runs at a time (AGENTS.md); the script refuses to start
# while another one runs. See book/CONTRIBUTING-BOOK.md, "The README demo".
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
image=volt-demo

# Docker on Windows takes C:\... paths; Git Bash would rewrite /demo.
export MSYS_NO_PATHCONV=1
host_path() {
    if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s\n' "$1"; fi
}

if [ -n "$(docker ps -q)" ]; then
    echo "record.sh: another container is running (docker ps); run one at a time" >&2
    exit 1
fi

git archive --format=tar HEAD | docker build -f demo/Dockerfile -t "$image" -

out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
demo_dir=$(host_path "$root/demo")
out_dir=$(host_path "$out")

docker run --rm -v "$demo_dir:/demo:ro" -v "$out_dir:/vhs/out" "$image" /demo/cdc.tape

# The edit typed in the recording must give the book's fixed file.
if ! cmp -s "$out/crossing_after.volt" demo/crossing_fixed.volt; then
    echo "record.sh: the file edited in the recording differs from demo/crossing_fixed.volt:" >&2
    diff "$out/crossing_after.volt" demo/crossing_fixed.volt >&2 || true
    exit 1
fi
cp "$out/cdc-demo.gif" demo/cdc-demo.gif

# The same two checks without the terminal, for the text under the GIF.
docker run --rm -v "$demo_dir:/demo:ro" --entrypoint sh "$image" -c '
    set -u
    cd /tmp
    cp /demo/crossing.volt crossing.volt
    echo "\$ volt check crossing.volt"
    NO_COLOR=1 volt check crossing.volt
    echo "(exit $?)"
    cp /demo/crossing_fixed.volt crossing.volt
    echo "\$ volt check crossing.volt"
    NO_COLOR=1 volt check crossing.volt
    echo "(exit $?)"
'

echo "record.sh: wrote demo/cdc-demo.gif ($(wc -c < demo/cdc-demo.gif) bytes)"
