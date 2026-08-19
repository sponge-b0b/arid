#!/usr/bin/env python3
"""Unit tests for qualification.pypi_ready."""

from __future__ import annotations

import unittest
import urllib.error

from qualification.pypi_ready import wait_for_release


class Response:
    def close(self) -> None:
        pass


def http_error(code: int) -> urllib.error.HTTPError:
    return urllib.error.HTTPError(
        url="https://pypi.org/pypi/arid/test/json",
        code=code,
        msg="test",
        hdrs=None,
        fp=None,
    )


class FakeClock:
    def __init__(self) -> None:
        self.now = 0.0

    def monotonic(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.now += seconds


class PyPIReadinessTests(unittest.TestCase):
    def test_succeeds_immediately_on_200(self) -> None:
        self.assertTrue(wait_for_release("1.2.0", open_url=lambda *_args, **_kwargs: Response()))

    def test_retries_404_then_succeeds(self) -> None:
        clock = FakeClock()
        attempts = 0

        def open_url(*_args: object, **_kwargs: object) -> Response:
            nonlocal attempts
            attempts += 1
            if attempts == 1:
                raise http_error(404)
            return Response()

        self.assertTrue(
            wait_for_release(
                "1.2.0",
                timeout=10,
                interval=1,
                open_url=open_url,
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        )
        self.assertEqual(attempts, 2)
        self.assertEqual(clock.now, 1.0)

    def test_returns_false_after_repeated_404_until_timeout(self) -> None:
        clock = FakeClock()

        def open_url(*_args: object, **_kwargs: object) -> Response:
            raise http_error(404)

        self.assertFalse(
            wait_for_release(
                "1.2.0",
                timeout=2,
                interval=0.5,
                open_url=open_url,
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        )
        self.assertEqual(clock.now, 2.0)

    def test_non_404_http_error_fails_immediately(self) -> None:
        clock = FakeClock()

        def open_url(*_args: object, **_kwargs: object) -> Response:
            raise http_error(500)

        with self.assertRaises(urllib.error.HTTPError):
            wait_for_release(
                "1.2.0",
                open_url=open_url,
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        self.assertEqual(clock.now, 0.0)

    def test_network_error_fails_immediately(self) -> None:
        clock = FakeClock()

        def open_url(*_args: object, **_kwargs: object) -> Response:
            raise urllib.error.URLError("offline")

        with self.assertRaises(urllib.error.URLError):
            wait_for_release(
                "1.2.0",
                open_url=open_url,
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        self.assertEqual(clock.now, 0.0)


if __name__ == "__main__":
    unittest.main()
