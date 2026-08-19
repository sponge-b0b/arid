#!/usr/bin/env python3
"""Wait for one exact Arid release to become visible on PyPI."""

from __future__ import annotations

import argparse
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable

DEFAULT_TIMEOUT = 120.0
DEFAULT_INTERVAL = 5.0
PYPI_URL = "https://pypi.org/pypi/arid/{version}/json"


def wait_for_release(
    version: str,
    *,
    timeout: float = DEFAULT_TIMEOUT,
    interval: float = DEFAULT_INTERVAL,
    open_url: Callable[..., object] = urllib.request.urlopen,
    monotonic: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], None] = time.sleep,
) -> bool:
    """Return True when PyPI serves the exact release, False after repeated 404s."""
    deadline = monotonic() + timeout
    url = PYPI_URL.format(version=version)

    while True:
        try:
            response = open_url(url, timeout=10)
            close = getattr(response, "close", None)
            if close is not None:
                close()
            return True
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise

        now = monotonic()
        if now >= deadline:
            return False

        sleep(min(interval, deadline - now))


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Wait for an exact Arid release to become visible on PyPI."
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
    except (urllib.error.HTTPError, urllib.error.URLError, OSError) as error:
        print(f"error: PyPI readiness check failed: {error}", file=sys.stderr)
        return 2

    if not ready:
        print(
            f"error: arid=={args.version} did not become visible on PyPI "
            f"within {args.timeout:g} seconds",
            file=sys.stderr,
        )
        return 1

    print(f"PyPI release ready: arid=={args.version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
