#!/usr/bin/env python3
"""Free disk space and what the build directories take (`just disk`).

AGENTS.md, "Machines with limited resources": check the free space before a
long local job and while it runs; below the limit (20 GB) stop the job.
This prints:

- the free space of the drive that holds the repository;
- the size of the `target/` directory of every worktree of the repository
  (`git worktree list`), and of `CARGO_TARGET_DIR` when it is set;
- the size of Docker Desktop's virtual disk, when one is found: it grows
  with what runs in containers and does not shrink by itself.

It exits with 1 when the free space is below the limit, so that a script
can stop on it, and with 0 otherwise. It only reads; it deletes nothing.

    python scripts/disk.py [--min-free-gb 20]
"""

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

GIB = 1024**3


def tree_size(root: Path) -> int:
    """Bytes of the files under root (symbolic links are not followed)."""
    total = 0
    stack = [root]
    while stack:
        d = stack.pop()
        try:
            with os.scandir(d) as it:
                for e in it:
                    try:
                        if e.is_dir(follow_symlinks=False):
                            stack.append(Path(e.path))
                        elif e.is_file(follow_symlinks=False):
                            total += e.stat(follow_symlinks=False).st_size
                    except OSError:
                        pass
        except OSError:
            pass
    return total


def allocated(path: Path) -> int:
    """The space a file takes on disk (a sparse file counts its blocks)."""
    st = path.stat()
    blocks = getattr(st, "st_blocks", None)
    return blocks * 512 if blocks is not None else st.st_size


def worktrees(repo: Path) -> list[tuple[Path, str]]:
    """(path, branch or "detached") of every worktree of the repository."""
    out = subprocess.run(
        ["git", "worktree", "list", "--porcelain"], cwd=repo, check=True, capture_output=True, text=True
    ).stdout
    result, path, branch = [], None, "detached"
    for line in out.splitlines() + [""]:
        if line.startswith("worktree "):
            path = Path(line[len("worktree ") :])
        elif line.startswith("branch "):
            branch = line[len("branch ") :].removeprefix("refs/heads/")
        elif not line and path is not None:
            result.append((path, branch))
            path, branch = None, "detached"
    return result


def docker_disks() -> list[Path]:
    """Docker Desktop's virtual disk files that exist on this machine."""
    home = Path.home()
    local = Path(os.environ.get("LOCALAPPDATA", home / "AppData" / "Local"))
    candidates = [
        local / "Docker" / "wsl" / "disk" / "docker_data.vhdx",
        local / "Docker" / "wsl" / "data" / "ext4.vhdx",
        home / "Library" / "Containers" / "com.docker.docker" / "Data" / "vms" / "0" / "data" / "Docker.raw",
    ]
    return [p for p in candidates if p.is_file()]


def gib(n: int) -> str:
    return f"{n / GIB:7.1f} GiB"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--min-free-gb", type=float, default=20.0, help="the limit in GiB (default 20)")
    args = ap.parse_args()
    repo = Path(__file__).resolve().parent.parent
    free = shutil.disk_usage(repo).free
    print(f"free disk: {gib(free)} on the drive of {repo} (limit {args.min_free_gb:g} GiB)")
    print("target/ of each worktree:")
    seen = set()
    for path, branch in worktrees(repo):
        target = path / "target"
        size = tree_size(target) if target.is_dir() else 0
        seen.add(os.path.normcase(str(target.resolve())) if target.exists() else None)
        print(f"  {gib(size)}  {path}  [{branch}]" + ("" if target.is_dir() else "  (no target/)"))
    env_target = os.environ.get("CARGO_TARGET_DIR")
    if env_target and Path(env_target).is_dir():
        if os.path.normcase(str(Path(env_target).resolve())) not in seen:
            print(f"  {gib(tree_size(Path(env_target)))}  CARGO_TARGET_DIR={env_target}")
    for disk in docker_disks():
        print(f"Docker virtual disk: {gib(allocated(disk))}  {disk}")
    if free < args.min_free_gb * GIB:
        print(f"free disk below {args.min_free_gb:g} GiB: stop long jobs and free space first", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
