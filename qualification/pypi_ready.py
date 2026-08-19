#!/usr/bin/env python3
"""Wait for one exact Arid release to become installable from PyPI."""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable

DEFAULT_TIMEOUT = 120.0
DEFAULT_INTERVAL = 5.0
PYPI_URL = "https://pypi.org/simple/arid/"
SIMPLE_API_ACCEPT = "application/vnd.pypi.simple.v1+json"


def release_is_listed(payload: object, version: str) -> bool:
    """Return whether the Simple API lists a distribution for the exact version."""
    if not isinstance(payload, dict):
        raise ValueError("PyPI Simple API response is not an object")

    files = payload.get("files")
    if not isinstance(files, list):
        raise ValueError("PyPI Simple API response is missing files")

    wheel_prefix = f"arid-{version}-"
    sdist_name = f"arid-{version}.tar.gz"

    for entry in files:
        if not isinstance(entry, dict):
            raise ValueError("PyPI Simple API contains an invalid file entry")

        filename = entry.get("filename")
        if not isinstance(filename, str):
            raise ValueError("PyPI Simple API file entry is missing filename")

        if filename.startswith(wheel_prefix) or filename == sdist_name:
            return True

    return False


def wait_for_release(
    version: str,
    *,
    timeout: float = DEFAULT_TIMEOUT,
    interval: float = DEFAULT_INTERVAL,
    open_url: Callable[..., object] = urllib.request.urlopen,
    monotonic: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], None] = time.sleep,
) -> bool:
    """Return True when pip's Simple API lists the exact release."""
    deadline = monotonic() + timeout
    request = urllib.request.Request(
        PYPI_URL,
        headers={"Accept": SIMPLE_API_ACCEPT},
    )

    while True:
        response = open_url(request, timeout=10)
        try:
            read = getattr(response, "read", None)
            if read is None:
                raise ValueError("PyPI Simple API response is not readable")
            payload = json.loads(read())
        finally:
            close = getattr(response, "close", None)
            if close is not None:
                close()

        if release_is_listed(payload, version):
            return True

        now = monotonic()
        if now >= deadline:
            return False

        sleep(min(interval, deadline - now))


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Wait for an exact Arid release to become installable from PyPI."
    )
    parser.add_argument("version", help="Exact PyPI version, for example 1.2.0rc1")
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    parser.add_argument("--interval", type=float, default=DEFAULT_INTERVAL)
    args = parser.parse_args(argv)

    if args.timeout < 0:
        parser.error("--timeout must be non-negative")
    if args.interval <= 0:
        parser.error("--interval must be positive")

    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)

    try:
        ready = wait_for_release(
            args.version,
            timeout=args.timeout,
            interval=args.interval,
        )
    except (
        json.JSONDecodeError,
        urllib.error.HTTPError,
        urllib.error.URLError,
        OSError,
        ValueError,
    ) as error:
        print(f"error: PyPI readiness check failed: {error}", file=sys.stderr)
        return 2

    if not ready:
        print(
            f"error: arid=={args.version} did not become installable from PyPI "
            f"within {args.timeout:g} seconds",
            file=sys.stderr,
        )
        return 1

    print(f"PyPI release ready: arid=={args.version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
