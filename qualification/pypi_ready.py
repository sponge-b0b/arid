#!/usr/bin/env python3
"""Wait for one exact Arid release to become installable from PyPI."""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable

DEFAULT_TIMEOUT = 120.0
DEFAULT_INTERVAL = 5.0
PYPI_INDEX_URL = "https://pypi.org/simple"


def pip_can_download(
    version: str,
    *,
    python: str = sys.executable,
    run: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
) -> bool:
    """Return whether pip can resolve a wheel for the exact release."""
    with tempfile.TemporaryDirectory(prefix="arid-pypi-ready-") as destination:
        command = [
            python,
            "-m",
            "pip",
            "download",
            "--isolated",
            "--disable-pip-version-check",
            "--no-cache-dir",
            "--no-deps",
            "--only-binary=:all:",
            "--index-url",
            PYPI_INDEX_URL,
            "--dest",
            destination,
            f"arid=={version}",
        ]
        completed = run(
            command,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )

    return completed.returncode == 0


def wait_for_release(
    version: str,
    *,
    timeout: float = DEFAULT_TIMEOUT,
    interval: float = DEFAULT_INTERVAL,
    probe: Callable[[str], bool] = pip_can_download,
    monotonic: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], None] = time.sleep,
) -> bool:
    """Return True when pip can download the exact release for this platform."""
    deadline = monotonic() + timeout

    while True:
        if probe(version):
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
    except (OSError, subprocess.SubprocessError) as error:
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
