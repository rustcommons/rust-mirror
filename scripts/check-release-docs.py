#!/usr/bin/env python3
"""Check local release documentation; optionally verify published GitHub notes.

Python 3.11+ and its standard library only. This does not publish anything.
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
from pathlib import Path
import re
import sys
import tomllib
import urllib.error
import urllib.request

VERSION = r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?"
ENTRY = re.compile(r"^## \[(" + VERSION + r")\]\s*[-—]\s*(\d{4}-\d{2}-\d{2})\s*$", re.M)
SECTIONS = ("Changes", "Upgrade", "Validation and limits", "Links")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def text(path: Path) -> str:
    require(path.is_file(), f"Missing required release document: {path}")
    return path.read_text(encoding="utf-8-sig")


def manifest_version(root: Path) -> str:
    manifest = tomllib.loads(text(root / "Cargo.toml"))
    value = manifest.get("package", {}).get("version")
    if isinstance(value, dict) and value.get("workspace") is True:
        value = manifest.get("workspace", {}).get("package", {}).get("version")
    require(isinstance(value, str) and re.fullmatch(VERSION, value) is not None,
            "Root Cargo package must have a resolved semantic version")
    return value


def markdown_targets(body: str) -> list[str]:
    """Conservatively check inline links/images and reference definitions."""
    targets = []
    for match in re.finditer(r"\]\(\s*(?:<([^>\r\n]+)>|([^\s)]+))", body):
        targets.append(match.group(1) or match.group(2))
    definitions = r"(?m)^ {0,3}\[(?!\^)(?:\\.|[^\]\\])+\]:[ \t]*(?:\r?\n[ \t]*)?(?:<([^>\r\n]+)>|(\S+))"
    for match in re.finditer(definitions, body):
        targets.append(match.group(1) or match.group(2))
    return targets


def local_check(root: Path, repo: str) -> tuple[str, dict[str, str]]:
    current = manifest_version(root)
    changelog = text(root / "CHANGELOG.md")
    require(re.search(r"^## \[Unreleased\]\s*$", changelog, re.M) is not None,
            "CHANGELOG.md needs an Unreleased section")
    entries = ENTRY.findall(changelog)
    require(bool(entries), "CHANGELOG.md needs dated ## [X.Y.Z] - YYYY-MM-DD entries")
    versions = [version for version, _ in entries]
    require(len(versions) == len(set(versions)), "Duplicate CHANGELOG.md version")
    require(current in versions, f"Current package {current} has no dated changelog entry")
    notes: dict[str, str] = {}
    for version, date in entries:
        datetime.date.fromisoformat(date)
        note = text(root / "docs" / "releases" / f"v{version}.md")
        headings = re.findall(r"^# (.+)$", note, re.M)
        require(bool(headings) and re.search(
            r"(?<![0-9.])v?" + re.escape(version) + r"(?![0-9.])", headings[0]) is not None,
            f"v{version} notes need a title identifying the version")
        for heading in SECTIONS:
            require(re.search(r"^## " + re.escape(heading) + r"\s*$", note, re.M) is not None,
                    f"v{version} notes need section: {heading}")
        require(f"https://github.com/{repo}/tree/v{version}" in note
                or f"https://github.com/{repo}/releases/tag/v{version}" in note,
                f"v{version} notes need an absolute source-tag or release link")
        require("CHANGELOG.md" in note, f"v{version} notes need a changelog link")
        for target in markdown_targets(note):
            require(target.startswith("#") or re.match(r"https?://", target, re.I) is not None,
                    f"v{version} release-body link must be absolute: {target}")
        notes[version] = note
    note_versions = {p.stem[1:] for p in (root / "docs" / "releases").glob("v*.md")}
    require(note_versions == set(versions),
            "Versioned notes and dated changelog entries must cover the same versions")
    return current, notes


def github(repo: str, endpoint: str):
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    headers = {"Accept": "application/vnd.github+json",
               "User-Agent": "rustcommons-release-documentation"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(f"https://api.github.com/repos/{repo}/{endpoint}",
                                     headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        raise ValueError(f"GitHub {endpoint} returned HTTP {error.code}") from None


def published_check(repo: str, current: str, notes: dict[str, str],
                    only_tag: str | None) -> None:
    if only_tag:
        require(only_tag.startswith("v") and only_tag[1:] in notes,
                f"Published tag {only_tag} has no reviewed local notes")
        selected = {only_tag[1:]: notes[only_tag[1:]]}
        releases = [github(repo, f"releases/tags/{only_tag}")]
    else:
        selected = notes
        releases = github(repo, "releases?per_page=100")
        require(len(releases) < 100,
                "Release history reached API page size; add pagination before claiming complete verification")
    by_tag = {release["tag_name"]: release for release in releases}
    tag_entries = github(repo, "tags?per_page=100")
    require(len(tag_entries) < 100,
            "Tag history reached API page size; add pagination before claiming complete verification")
    tags = {entry["name"]: entry for entry in tag_entries}
    if not only_tag:
        version_tag = re.compile("v" + VERSION)
        remote_version_tags = {tag for tag in tags if version_tag.fullmatch(tag)}
        remote_release_tags = {tag for tag in by_tag if version_tag.fullmatch(tag)}
        documented_tags = {f"v{version}" for version in notes}
        missing = sorted((remote_version_tags | remote_release_tags) - documented_tags)
        require(not missing, "Existing version tags/releases have no reviewed local notes: "
                + ", ".join(missing))
        other_tags = (set(tags) | set(by_tag)) - remote_version_tags - remote_release_tags
        if other_tags:
            print("Non-version tags/channels outside this versioned-history check: "
                  + ", ".join(sorted(other_tags)))
    for version, expected in selected.items():
        tag = f"v{version}"
        require(tag in tags, f"Missing existing source tag: {tag}")
        require(tag in by_tag, f"Missing published GitHub Release: {tag}")
        release = by_tag[tag]
        require(not release["draft"], f"{tag} is still a draft")
        require(release["html_url"] == f"https://github.com/{repo}/releases/tag/{tag}",
                f"Unexpected release URL for {tag}")
        actual = (release.get("body") or "").replace("\r\n", "\n").strip()
        require(actual == expected.replace("\r\n", "\n").strip(),
                f"Published {tag} body differs from its reviewed notes file")
    if not only_tag and "-" not in current:
        latest = github(repo, "releases/latest")
        require(latest["tag_name"] == f"v{current}",
                "Latest GitHub Release must identify the current stable package version")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--repo", required=True, help="GitHub owner/repository")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--published", action="store_true",
                       help="Verify all note bodies and latest release against GitHub")
    group.add_argument("--published-tag", help="Verify one published release event")
    args = parser.parse_args()
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repo) is not None,
            "Expected GitHub owner/repository")
    current, notes = local_check(args.root.resolve(), args.repo)
    if args.published or args.published_tag:
        published_check(args.repo, current, notes, args.published_tag)
    mode = "local and published" if args.published or args.published_tag else "local"
    print(f"Release documentation PASS: {args.repo}, {len(notes)} versions, current {current}, {mode}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, tomllib.TOMLDecodeError) as error:
        print(f"Release documentation FAIL: {error}", file=sys.stderr)
        sys.exit(1)
