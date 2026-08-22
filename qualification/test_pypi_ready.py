#!/usr/bin/env python3
"""Unit tests for qualification.pypi_ready."""

from __future__ import annotations

import subprocess
import unittest

from qualification.pypi_ready import PYPI_INDEX_URL, pip_can_download, wait_for_release


class FakeClock:
    def __init__(self) -> None:
        self.now = 0.0

    def monotonic(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.now += seconds


class PyPIReadinessTests(unittest.TestCase):
    def test_pip_probe_uses_exact_release_and_installer_index(self) -> None:
        captured_command: list[str] = []
        captured_kwargs: dict[str, object] = {}

        def run(
            command: list[str],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[bytes]:
            captured_command.extend(command)
            captured_kwargs.update(kwargs)
            return subprocess.CompletedProcess(command, 0)

        self.assertTrue(
            pip_can_download(
                "2.0.0b1",
                python="/test/python",
                run=run,
            )
        )
        self.assertEqual(captured_command[:4], ["/test/python", "-m", "pip", "download"])
        self.assertIn("--isolated", captured_command)
        self.assertIn("--no-cache-dir", captured_command)
        self.assertIn("--no-deps", captured_command)
        self.assertIn("--only-binary=:all:", captured_command)
        self.assertEqual(
            captured_command[captured_command.index("--index-url") + 1],
            PYPI_INDEX_URL,
        )
        self.assertEqual(captured_command[-1], "arid==2.0.0b1")
        self.assertFalse(captured_kwargs["check"])
        self.assertEqual(captured_kwargs["stdout"], subprocess.DEVNULL)
        self.assertEqual(captured_kwargs["stderr"], subprocess.DEVNULL)

    def test_pip_probe_returns_false_when_exact_release_is_unavailable(self) -> None:
        def run(
            command: list[str],
            **_kwargs: object,
        ) -> subprocess.CompletedProcess[bytes]:
            return subprocess.CompletedProcess(command, 1)

        self.assertFalse(pip_can_download("2.0.0b1", run=run))

    def test_succeeds_immediately_when_pip_can_download_exact_version(self) -> None:
        attempts: list[str] = []

        def probe(version: str) -> bool:
            attempts.append(version)
            return True

        self.assertTrue(wait_for_release("2.0.0b1", probe=probe))
        self.assertEqual(attempts, ["2.0.0b1"])

    def test_retries_until_pip_can_download_exact_version(self) -> None:
        clock = FakeClock()
        attempts = 0

        def probe(version: str) -> bool:
            nonlocal attempts
            self.assertEqual(version, "2.0.0b1")
            attempts += 1
            return attempts >= 2

        self.assertTrue(
            wait_for_release(
                "2.0.0b1",
                timeout=10,
                interval=1,
                probe=probe,
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        )
        self.assertEqual(attempts, 2)
        self.assertEqual(clock.now, 1.0)

    def test_returns_false_when_pip_never_sees_exact_version(self) -> None:
        clock = FakeClock()
        attempts = 0

        def probe(version: str) -> bool:
            nonlocal attempts
            self.assertEqual(version, "2.0.0b1")
            attempts += 1
            return False

        self.assertFalse(
            wait_for_release(
                "2.0.0b1",
                timeout=2,
                interval=0.5,
                probe=probe,
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        )
        self.assertEqual(attempts, 5)
        self.assertEqual(clock.now, 2.0)

    def test_probe_error_propagates_without_retry(self) -> None:
        clock = FakeClock()

        def probe(_version: str) -> bool:
            raise OSError("pip unavailable")

        with self.assertRaisesRegex(OSError, "pip unavailable"):
            wait_for_release(
                "2.0.0b1",
                probe=probe,
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        self.assertEqual(clock.now, 0.0)


if __name__ == "__main__":
    unittest.main()
