#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Shared helpers for the public-migration tooling.

The tooling is generic: repository URLs, tokens and internal forbidden strings
are passed in, never hardcoded, so the published copy does not leak them.
"""

import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

USER_AGENT = "rawboot-migrate/1"
GITHUB_API_VERSION = "2026-03-10"


def read_token(env_name, token_file=None):
    """Return a token from the environment or an explicit file, or None."""
    token = os.environ.get(env_name, "").strip()
    if token:
        return token
    if token_file:
        path = Path(token_file).expanduser()
        if path.is_file():
            return path.read_text(encoding="utf-8").strip()
        raise SystemExit(f"error: token file not found: {path}")
    return None


def _request(url, token=None, accept="application/json", api_version=None):
    headers = {"User-Agent": USER_AGENT, "Accept": accept}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if api_version:
        headers["X-GitHub-Api-Version"] = api_version
    request = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            body = response.read()
            return json.loads(body.decode("utf-8")), response.headers
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", "replace")[:300]
        raise SystemExit(
            f"error: GET {url} -> HTTP {error.code}: {detail}"
        ) from error
    except urllib.error.URLError as error:
        raise SystemExit(f"error: GET {url} failed: {error.reason}") from error


def get_json(url, token=None, api_version=None):
    data, _ = _request(url, token, api_version=api_version)
    return data


def get_json_optional(url, token=None, api_version=None):
    """Like get_json, but a missing endpoint (HTTP 404) yields an empty list."""
    try:
        return get_json(url, token, api_version=api_version)
    except SystemExit as error:
        if "HTTP 404" in str(error):
            print(f"warning: {url} is unavailable; using an empty list", file=sys.stderr)
            return []
        raise


def _with_query(url, **params):
    separator = "&" if "?" in url else "?"
    encoded = urllib.parse.urlencode(params)
    return f"{url}{separator}{encoded}"


def get_gitea_all(url, token, limit=50):
    """Fetch every page of a Gitea list endpoint."""
    items = []
    page = 1
    while True:
        page_url = _with_query(url, page=page, limit=limit)
        batch = get_json(page_url, token)
        if not isinstance(batch, list):
            raise SystemExit(f"error: expected a list from {page_url}")
        items.extend(batch)
        if len(batch) < limit:
            return items
        page += 1


def _next_link(headers):
    link = headers.get("Link", "")
    match = re.search(r'<([^>]+)>;\s*rel="next"', link)
    return match.group(1) if match else None


def get_github_all(url, token=None):
    """Fetch every page of a GitHub list endpoint by following Link."""
    items = []
    while url:
        batch, headers = _request(
            url,
            token,
            accept="application/vnd.github+json",
            api_version=GITHUB_API_VERSION,
        )
        if not isinstance(batch, list):
            raise SystemExit(f"error: expected a list from {url}")
        items.extend(batch)
        url = _next_link(headers)
    return items


def normalize_body(text):
    """Strip a Markdown body to the form both trackers return."""
    if text is None:
        return ""
    return text.replace("\r\n", "\n").strip()


def compare_body(left, right):
    """Return (line_number, left_line, right_line) for the first difference."""
    left_lines = normalize_body(left).split("\n")
    right_lines = normalize_body(right).split("\n")
    for index in range(max(len(left_lines), len(right_lines))):
        left_line = left_lines[index] if index < len(left_lines) else "<missing>"
        right_line = right_lines[index] if index < len(right_lines) else "<missing>"
        if left_line != right_line:
            return index + 1, left_line, right_line
    return None


def now_iso():
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def write_json(path, data):
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
    target.write_text(text, encoding="utf-8")
    print(f"wrote {target} ({len(data.get('items', []))} items)", file=sys.stderr)
