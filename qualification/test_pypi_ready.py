#!/usr/bin/env python3
"""Unit tests for qualification.pypi_ready."""

from __future__ import annotations

import json
import unittest
import urllib.error
import urllib.request

from qualification.pypi_ready import PYPI_URL, SIMPLE_API_ACCEPT, wait_for_release


class Response:
    def __init__(self, filenames: list[str]) -> None:
        self.body = json.dumps(
            {"files": [{"filename": filename} for filename in filenames]}
        ).encode()

    def read(self) -> bytes:
        return self.body

    def close(self) -> None:
        pass


def http_error(code: int) -> urllib.error.HTTPError:
    return urllib.error.HTTPError(
        url=PYPI_URL,
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
    def test_succeeds_immediately_when_exact_version_is_listed(self) -> None:
        def open_url(request: object, **_kwargs: object) -> Response:
            self.assertIsInstance(request, urllib.request.Request)
            assert isinstance(request, urllib.request.Request)
            self.assertEqual(request.full_url, PYPI_URL)
            self.assertEqual(request.get_header("Accept"), SIMPLE_API_ACCEPT)
            return Response(["arid-1.2.0-py3-none-manylinux_2_17_x86_64.whl"])

        self.assertTrue(wait_for_release("1.2.0", open_url=open_url))

    def test_retries_missing_version_then_succeeds(self) -> None:
        clock = FakeClock()
        attempts = 0

        def open_url(*_args: object, **_kwargs: object) -> Response:
            nonlocal attempts
            attempts += 1
            if attempts == 1:
                return Response(["arid-1.2.0rc1-py3-none-manylinux_2_17_x86_64.whl"])
            return Response(["arid-1.2.0-py3-none-manylinux_2_17_x86_64.whl"])

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

    def test_returns_false_when_exact_version_remains_absent(self) -> None:
        clock = FakeClock()

        def open_url(*_args: object, **_kwargs: object) -> Response:
            return Response(["arid-1.2.0rc1-py3-none-manylinux_2_17_x86_64.whl"])

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

    def test_http_error_fails_immediately(self) -> None:
        clock = FakeClock()

        def open_url(*_args: object, **_kwargs: object) -> Response:
            raise http_error(404)

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

    def test_invalid_simple_api_payload_fails_immediately(self) -> None:
        clock = FakeClock()

        class InvalidResponse:
            def read(self) -> bytes:
                return b'{"files": "invalid"}'

            def close(self) -> None:
                pass

        with self.assertRaises(ValueError):
            wait_for_release(
                "1.2.0",
                open_url=lambda *_args, **_kwargs: InvalidResponse(),
                monotonic=clock.monotonic,
                sleep=clock.sleep,
            )
        self.assertEqual(clock.now, 0.0)


if __name__ == "__main__":
    unittest.main()
