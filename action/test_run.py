from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from action import run as action_run


class ActionHelperTests(unittest.TestCase):
    def test_split_lines_handles_paths_and_focus(self) -> None:
        self.assertEqual(
            action_run.split_lines("src\n\ntests/unit\n"),
            ["src", "tests/unit"],
        )
        self.assertEqual(action_run.split_lines("", default=(".",)), ["."])

    def test_shell_style_additional_arguments(self) -> None:
        self.assertEqual(
            action_run.parse_additional_arguments(
                "--min-lines 6 --exclude 'generated files/**' --workers auto"
            ),
            [
                "--min-lines",
                "6",
                "--exclude",
                "generated files/**",
                "--workers",
                "auto",
            ],
        )

    def test_administrative_modes_are_rejected(self) -> None:
        for option in sorted(action_run.ADMINISTRATIVE_OPTIONS):
            with self.subTest(option=option):
                with self.assertRaises(action_run.ActionError):
                    action_run.validate_action_arguments([option])

        with self.assertRaises(action_run.ActionError):
            action_run.validate_action_arguments(["--baseline-status=baseline.json"])

    def test_option_like_path_after_separator_is_not_rejected(self) -> None:
        action_run.validate_action_arguments(["--", "--show-config"])

    def test_virtual_stdin_is_rejected(self) -> None:
        with self.assertRaises(action_run.ActionError):
            action_run.validate_action_arguments(["--stdin-path", "proposed.py"])

    def test_build_command_adds_internal_reports_to_one_scan(self) -> None:
        command = action_run.build_command(
            ["src", "tests"],
            ["src/app.py", "tests/unit"],
            ["--workers", "auto"],
            Path("/tmp/report.json"),
            summary_path=Path("/tmp/report.md"),
            sarif_path=Path("/tmp/report.sarif"),
        )

        self.assertEqual(command[0], "arid")
        self.assertEqual(command.count("arid"), 1)
        self.assertEqual(command[1:3], ["src", "tests"])
        self.assertIn("json=/tmp/report.json", command)
        self.assertIn("markdown=/tmp/report.md", command)
        self.assertIn("sarif=/tmp/report.sarif", command)
        self.assertEqual(command[-2:], ["--workers", "auto"])
        self.assertEqual(command.count("--focus"), 2)

    def test_report_v4_metrics_are_extracted_without_recomputation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 4,
                        "tool_version": "2.0.0-alpha.1",
                        "complete": True,
                        "files": 12,
                        "duplicate_groups": 3,
                        "duplicate_lines": 18,
                        "duplication_percent": 4.25,
                    }
                ),
                encoding="utf-8",
            )

            metrics = action_run.read_report(path)

        self.assertEqual(metrics.tool_version, "2.0.0-alpha.1")
        self.assertTrue(metrics.complete)
        self.assertEqual(metrics.files, 12)
        self.assertEqual(metrics.duplicate_groups, 3)
        self.assertEqual(metrics.duplicate_lines, 18)
        self.assertEqual(metrics.duplication_percent, 4.25)
        self.assertTrue(metrics.has_findings)

    def test_exit_policy_maps_findings_and_errors(self) -> None:
        clean = action_run.ReportMetrics("2.0.0", True, 2, 0, 0, 0.0)
        findings = action_run.ReportMetrics("2.0.0", True, 2, 1, 4, 10.0)
        incomplete = action_run.ReportMetrics("2.0.0", False, 1, 1, 2, 5.0)

        self.assertFalse(action_run.should_fail(0, clean, True))
        self.assertTrue(action_run.should_fail(1, findings, True))
        self.assertFalse(action_run.should_fail(1, findings, False))
        self.assertFalse(action_run.should_fail(0, findings, False))
        self.assertTrue(action_run.should_fail(2, incomplete, False))
        self.assertTrue(action_run.should_fail(2, None, False))

    def test_incomplete_report_suppresses_sarif(self) -> None:
        complete = action_run.ReportMetrics("2.0.0", True, 2, 1, 4, 10.0)
        incomplete = action_run.ReportMetrics("2.0.0", False, 1, 1, 2, 5.0)

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.sarif"
            path.write_text("{}", encoding="utf-8")

            self.assertTrue(action_run.sarif_ready(True, complete, path))
            self.assertFalse(action_run.sarif_ready(True, incomplete, path))
            self.assertFalse(action_run.sarif_ready(False, complete, path))

    def test_github_output_encoding_handles_multiline_values(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "output"
            path.touch()

            action_run.write_github_outputs(
                path,
                {"plain": "value", "multi": "first\nsecond"},
            )
            text = path.read_text(encoding="utf-8")

        self.assertIn("plain=value\n", text)
        self.assertIn("multi<<arid_", text)
        self.assertIn("\nfirst\nsecond\n", text)


if __name__ == "__main__":
    unittest.main()
