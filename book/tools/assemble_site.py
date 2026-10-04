#!/usr/bin/env python3
"""Assemble the GitHub Pages site from built books (ADR-0100, section 6).

    /       the book of the latest release tag (vX.Y.Z), no banner;
            without a release tag, the book of main with a "pre-release" banner
    /dev/   the book of main, always, with a "development version" banner

The install scripts are copied to the site root by book.yml, from main.

Usage:
    assemble_site.py --main book/book --out SITE [--release DIR --tag vX.Y.Z]
"""

from __future__ import annotations

import argparse
import html
import os
import re
import shutil
import sys
from pathlib import Path

RELEASE_TAG_RE = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+$")
MAIN_TAG = "<main>"


def banner(title: str, body: str) -> str:
    # mdBook's own warning admonition markup, so it follows the theme.
    return (
        '<blockquote class="blockquote-tag blockquote-tag-warning volt-version-banner">'
        f'<p class="blockquote-tag-title">{html.escape(title)}</p>'
        f"<p>{body}</p></blockquote>"
    )


def add_banner(site: Path, make_html, skip: Path | None = None, keep_404: bool = False) -> int:
    """Put a banner at the top of <main> in every page under `site`.

    `make_html` gets the relative path from the page to `site` ("." or
    "../..") so links in the banner work at any depth and on any host.
    Pages without <main> (mdBook's toc.html fragment) are left alone, and
    so is 404.html with `keep_404`: below the site root GitHub Pages never
    serves it, and mdBook writes <base href="/volt/"> into it, which would
    send the banner's relative link to the wrong place.
    """
    count = 0
    for page in sorted(site.rglob("*.html")):
        if skip is not None and skip in page.parents:
            continue
        if keep_404 and page.name == "404.html":
            continue
        text = page.read_text(encoding="utf-8")
        if MAIN_TAG not in text:
            continue
        to_root = Path(os.path.relpath(site, page.parent)).as_posix()
        text = text.replace(MAIN_TAG, MAIN_TAG + make_html(to_root), 1)
        page.write_text(text, encoding="utf-8")
        count += 1
    if not (site / "index.html").is_file() or count == 0:
        sys.exit(f"error: no book page with {MAIN_TAG} under {site}")
    return count


def prerelease_banner(to_root: str) -> str:
    return banner(
        "Pre-release",
        "No Volt release has been published yet. This book describes the "
        "development version (the main branch), and the install commands "
        "have nothing to download until v0.1.0 is out.",
    )


def dev_banner(tag: str | None):
    def make(to_root: str) -> str:
        if tag:
            latest = (
                f'The book for the latest release, {html.escape(tag)}, is '
                f'<a href="{to_root}/../index.html">here</a>.'
            )
        else:
            latest = "No release has been published yet."
        return banner(
            "Development version",
            "This book follows the main branch and can describe features "
            f"that are not in a release yet. {latest}",
        )

    return make


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--main", required=True, type=Path, help="built book of main")
    ap.add_argument("--release", type=Path, help="built book of the release tag")
    ap.add_argument("--tag", help="the release tag (vX.Y.Z) the --release book comes from")
    ap.add_argument("--out", required=True, type=Path, help="site directory (replaced)")
    args = ap.parse_args()

    if (args.release is None) != (args.tag is None):
        ap.error("--release and --tag go together")
    if args.tag is not None and not RELEASE_TAG_RE.match(args.tag):
        ap.error(f"--tag {args.tag!r} is not a release tag (vX.Y.Z)")
    for book in filter(None, [args.main, args.release]):
        if not (book / "index.html").is_file():
            ap.error(f"{book} is not a built book (no index.html)")

    out: Path = args.out
    # --out is deleted and rewritten: it must not hold or sit inside a book.
    for book in filter(None, [args.main, args.release]):
        a, b = out.resolve(), book.resolve()
        if a == b or a in b.parents or b in a.parents:
            ap.error(f"--out {out} overlaps the book {book}")
    if out.exists():
        shutil.rmtree(out)
    dev = out / "dev"

    if args.release is not None:
        shutil.copytree(args.release, out)
        root_note = f"book of {args.tag}"
    else:
        shutil.copytree(args.main, out)
        n = add_banner(out, prerelease_banner, skip=dev)
        root_note = f"book of main, pre-release banner on {n} pages"
    if dev.exists():
        sys.exit("error: the book at the site root already has a dev/ directory")
    shutil.copytree(args.main, dev)
    n = add_banner(dev, dev_banner(args.tag), keep_404=True)

    print(f"site root: {root_note}")
    print(f"site /dev/: book of main, development banner on {n} pages")
    return 0


if __name__ == "__main__":
    sys.exit(main())
