from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from action import run as action_run

ROOT = Path(__file__).resolve().parents[1]


def analysis(*, keep_going: bool = False) -> dict[str, object]:
    return {
        "min_lines": 4,
        "ignore_comments": True,
        "ignore_docstrings": True,
        "ignore_imports": True,
        "ignore_signatures": True,
        "same_file": True,
        "hidden": False,
        "exclude": [],
        "baseline_enabled": False,
        "focus": ["src/a.py"],
        "virtual_source": None,
        "keep_going": keep_going,
    }


def summary_report() -> dict[str, object]:
    return {
        "schema_version": 4,
        "tool_version": "2.2.0",
        "complete": True,
        "analysis": analysis(),
        "errors": [],
        "files": 8,
        "source_lines": 800,
        "analyzed_lines": 600,
        "duplicate_groups": 4,
        "duplicate_lines": 120,
        "duplication_percent": 20.0,
        "findings": [
            {
                "context": "executable",
                "scope": "function",
                "distribution": "cross-file",
                "occurrences": 2,
                "locations": [{"path": "src/a.py"}, {"path": "src/b.py"}],
            },
            {
                "context": "declarative",
                "scope": "module",
                "distribution": "same-file",
                "occurrences": 2,
                "locations": [{"path": "src/a.py"}, {"path": "src/a.py"}],
            },
            {
                "context": "mixed",
                "scope": "class",
                "distribution": "hybrid",
                "occurrences": 3,
                "locations": [
                    {"path": "src/a.py"},
                    {"path": "src/a.py"},
                    {"path": "src/c.py"},
                ],
            },
            {
                "context": "executable",
                "scope": "mixed",
                "distribution": "cross-file",
                "occurrences": 2,
                "locations": [{"path": "src/b.py"}, {"path": "src/d.py"}],
            },
        ],
    }


def report_from_summary_fixture(summary: dict[str, object]) -> dict[str, object]:
    source_analysis = dict(summary["analysis"])
    source_analysis.pop("ignore_files")
    return {
        "schema_version": 4,
        "tool_version": summary["tool_version"],
        "complete": summary["complete"],
        "analysis": source_analysis,
        "errors": summary["errors"],
        "files": summary["files"],
        "source_lines": summary["source_lines"],
        "analyzed_lines": summary["analyzed_lines"],
        "duplicate_groups": summary["duplicate_groups"],
        "duplicate_lines": summary["duplicate_lines"],
        "duplication_percent": summary["duplication_percent"],
        "findings": [],
    }


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

    def test_informational_options_are_rejected(self) -> None:
        for option in sorted(action_run.INFORMATIONAL_OPTIONS):
            with self.subTest(option=option):
                with self.assertRaises(action_run.ActionError):
                    action_run.validate_action_arguments([option])

    def test_option_like_path_after_separator_is_not_rejected(self) -> None:
        action_run.validate_action_arguments(["--", "--show-config"])
        self.assertFalse(action_run.has_option(["--", "--no-ignore-files"], "--no-ignore-files"))
        self.assertTrue(action_run.has_option(["--no-ignore-files"], "--no-ignore-files"))

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

    def test_summary_projection_matches_core_counting_semantics(self) -> None:
        summary = action_run.derive_summary(summary_report(), ignore_files=True)

        self.assertEqual(summary["occurrences"], 9)
        self.assertEqual(summary["files_with_duplicates"], 4)
        self.assertEqual(
            summary["context"],
            {"executable": 2, "declarative": 1, "mixed": 1},
        )
        self.assertEqual(
            summary["scope"],
            {"function": 1, "module": 1, "class": 1, "mixed": 1},
        )
        self.assertEqual(
            summary["distribution"],
            {"cross_file": 2, "same_file": 1, "hybrid": 1},
        )
        self.assertEqual(
            summary["hotspots"],
            [
                {"path": "src/a.py", "groups": 3, "occurrences": 5},
                {"path": "src/b.py", "groups": 2, "occurrences": 2},
                {"path": "src/c.py", "groups": 1, "occurrences": 1},
                {"path": "src/d.py", "groups": 1, "occurrences": 1},
            ],
        )
        self.assertEqual(summary["analysis"]["focus"], ["src/a.py"])
        self.assertTrue(summary["analysis"]["ignore_files"])

    def test_summary_projection_uses_deterministic_top_five(self) -> None:
        document = summary_report()
        findings = []
        for path in ["f.py", "e.py", "d.py", "c.py", "b.py", "a.py"]:
            findings.append(
                {
                    "context": "executable",
                    "scope": "function",
                    "distribution": "same-file",
                    "occurrences": 2,
                    "locations": [{"path": path}, {"path": path}],
                }
            )
        document.update(
            {
                "duplicate_groups": 6,
                "duplicate_lines": 6,
                "duplication_percent": 1.0,
                "findings": findings,
            }
        )

        summary = action_run.derive_summary(document, ignore_files=True)

        self.assertEqual(summary["files_with_duplicates"], 6)
        self.assertEqual(
            [hotspot["path"] for hotspot in summary["hotspots"]],
            ["a.py", "b.py", "c.py", "d.py", "e.py"],
        )

    def test_zero_and_incomplete_projection_match_core_summary_fixtures(self) -> None:
        for name in ["summary-v1-zero.json", "summary-v1-incomplete.json"]:
            with self.subTest(name=name):
                expected = json.loads(
                    (ROOT / "schemas" / "fixtures" / name).read_text(encoding="utf-8")
                )
                report = report_from_summary_fixture(expected)
                actual = action_run.derive_summary(
                    report,
                    ignore_files=expected["analysis"]["ignore_files"],
                )
                self.assertEqual(actual, expected)

    def test_no_ignore_files_policy_is_reflected_in_summary_analysis(self) -> None:
        summary = action_run.derive_summary(summary_report(), ignore_files=False)
        self.assertFalse(summary["analysis"]["ignore_files"])

    def test_action_outputs_expose_compact_summary(self) -> None:
        summary = action_run.derive_summary(summary_report(), ignore_files=True)
        metrics = action_run.report_metrics(summary_report())

        with tempfile.TemporaryDirectory() as directory:
            outputs = action_run.action_outputs(
                1,
                metrics,
                summary,
                False,
                Path(directory) / "unused.sarif",
                False,
            )

        self.assertEqual(outputs["occurrences"], "9")
        self.assertEqual(outputs["files-with-duplicates"], "4")
        self.assertEqual(json.loads(outputs["summary-json"]), summary)
        self.assertNotIn("\n", outputs["summary-json"])

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
