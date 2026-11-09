#!/usr/bin/env python3
"""Publish one completed snapshot with HTTP PUT, then verify every remote byte.

Compatible with Nexus Raw hosted URLs; not a Nexus repository provisioning API.
Python 3.11+. No external modules. Never logs authentication values.
"""
import argparse
import base64
import hashlib
import http.client
import os
from pathlib import Path
import stat
import tomllib
from urllib.parse import urlsplit


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def local_files(root):
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("snapshot root must be a real directory")
    inventory = root / "snapshot.toml"
    if inventory.is_symlink() or not inventory.is_file() or inventory.stat().st_size > 1024 * 1024:
        raise ValueError("missing or oversized snapshot inventory")
    document = tomllib.loads(inventory.read_text(encoding="utf-8"))
    if document.get("schema") != 1 or document.get("profile") != "minimal":
        raise ValueError("unsupported snapshot schema/profile")
    files = document.get("files", [])
    if not 1 <= len(files) <= 24:
        raise ValueError("invalid inventory file count")
    seen = set()
    for item in files:
        name = item["path"]
        if not isinstance(name, str) or len(name) > 512 or any(p in ("", ".", "..") for p in name.split("/")) or any(not(c.isascii() and (c.isalnum() or c in "-._/")) for c in name):
            raise ValueError("unsafe snapshot path")
        if name in seen or name == "snapshot.toml":
            raise ValueError("duplicate/reserved snapshot path")
        seen.add(name)
        target = root
        for part in name.split("/"):
            target = target / part
            if target.is_symlink():
                raise ValueError("snapshot paths must not contain symlinks")
        if not stat.S_ISREG(target.stat().st_mode) or target.stat().st_size != item["bytes"] or target.stat().st_size > 512 * 1024 * 1024 or digest(target) != item["sha256"]:
            raise ValueError(f"snapshot verification failed: {name}")
    actual = set()
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in dirs + names:
            target = Path(directory) / name
            if target.is_symlink():
                raise ValueError("unexpected symlink in snapshot")
        for name in names:
            target = Path(directory) / name
            if not stat.S_ISREG(target.stat().st_mode):
                raise ValueError("unexpected non-regular file")
            actual.add(target.relative_to(root).as_posix())
    if actual != seen | {"snapshot.toml"}:
        raise ValueError("unexpected or missing snapshot files")
    # Artifacts first; manifests and their checksums late; inventory is the last marker.
    ordered = sorted(seen, key=lambda n: (n.startswith("dist/channel-"), n.endswith(".sha256"), n))
    ordered.append("snapshot.toml")
    return document, [(n, root / n, digest(root / n)) for n in ordered]


def publish(root, base_url, allow_loopback_http=False):
    document, files = local_files(root)  # Verify everything before any request.
    base = urlsplit(base_url.rstrip("/"))
    if base.username or base.password or base.query or base.fragment or not base.hostname:
        raise ValueError("base URL must not contain credentials, query or fragment")
    if any(p in (".", "..") for p in base.path.split("/")) or any(not(c.isascii() and (c.isalnum() or c in "-._/")) for c in base.path):
        raise ValueError("unsafe base URL path")
    local_http = base.scheme == "http" and base.hostname in ("127.0.0.1", "::1") and allow_loopback_http
    if base.scheme != "https" and not local_http:
        raise ValueError("HTTPS required (literal loopback HTTP needs explicit test opt-in)")
    headers = {}
    user, password = os.environ.get("NEXUS_USERNAME"), os.environ.get("NEXUS_PASSWORD")
    if user or password:
        if not user or not password or base.scheme != "https":
            raise ValueError("both credentials and HTTPS are required for authentication")
        headers["Authorization"] = "Basic " + base64.b64encode(f"{user}:{password}".encode()).decode()
    connect = http.client.HTTPSConnection if base.scheme == "https" else http.client.HTTPConnection

    def request(method, name, body=None, expected=None):
        connection = connect(base.hostname, base.port, timeout=120)
        try:
            extra = dict(headers)
            if body is not None:
                extra.update({"Content-Length": str(body.stat().st_size), "Content-Type": "application/octet-stream", "If-None-Match": "*"})
            if body is not None:
                with body.open("rb") as stream:
                    connection.request(method, base.path + "/" + name, body=stream, headers=extra)
            else:
                connection.request(method, base.path + "/" + name, headers=extra)
            response = connection.getresponse()
            if method == "HEAD":
                if response.status != 404:
                    raise ValueError(f"destination is not absent: {name} (HTTP {response.status})")
                return
            if method == "PUT":
                if response.status not in (200, 201, 204):
                    raise ValueError(f"upload failed: {name} (HTTP {response.status})")
                return
            if response.status != 200:
                raise ValueError(f"verification GET failed: {name} (HTTP {response.status})")
            h = hashlib.sha256()
            total = 0
            while chunk := response.read(1024 * 1024):
                total += len(chunk)
                if total > 512 * 1024 * 1024:
                    raise ValueError("remote file exceeds byte budget")
                h.update(chunk)
            if (total, h.hexdigest()) != expected:
                raise ValueError(f"remote verification failed: {name}")
        finally:
            connection.close()

    # No implicit overwrite or resume. Use a fresh, privately controlled snapshot prefix.
    for name, path, sha in files:
        request("HEAD", name)
    for name, path, sha in files:
        request("PUT", name, body=path)
        request("GET", name, expected=(path.stat().st_size, sha))
        print(f"Verified upload: {name}", flush=True)
    return {"release": document["release"], "host": document["host"], "files": len(files)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--base-url", required=True, help="Fresh private Raw repository/prefix URL")
    parser.add_argument("--allow-loopback-http", action="store_true", help="Local protocol tests only")
    args = parser.parse_args()
    try:
        publish(args.source, args.base_url, args.allow_loopback_http)
    except Exception as error:
        # Connection errors can contain hostnames; never include credential headers.
        parser.exit(1, f"Raw publication failed: {error}\n")


if __name__ == "__main__":
    main()
