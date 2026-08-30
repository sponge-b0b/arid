#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import shlex
import subprocess
import sys
import tempfile
import uuid
from dataclasses import dataclass
from pathlib import Path

ADMINISTRATIVE_OPTIONS = frozenset(
    {
        "--baseline-status",
        "--capabilities",
        "--list-files",
        "--prune-baseline",
        "--show-config",
        "--write-baseline",
    }
)
INFORMATIONAL_OPTIONS = frozenset({"--help", "--version", "-V", "-h"})
UNSUPPORTED_ACTION_OPTIONS = frozenset({"--stdin-path"})


class ActionError(RuntimeError):
    pass


@dataclass(frozen=True)
class ReportMetrics:
    tool_version: str
    complete: bool
    files: int
    duplicate_groups: int
    duplicate_lines: int
    duplication_percent: float

    @property
    def has_findings(self) -> bool:
        return self.duplicate_groups > 0


def parse_bool(value: str, name: str) -> bool:
    normalized = value.strip().lower()
    if normalized == "true":
        return True
    if normalized == "false":
        return False
    raise ActionError(f"{name} must be 'true' or 'false'")


def split_lines(value: str, *, default: tuple[str, ...] = ()) -> list[str]:
    values = [line.strip() for line in value.splitlines() if line.strip()]
    return values or list(default)


def parse_additional_arguments(value: str) -> list[str]:
    try:
        return shlex.split(value, posix=True)
    except ValueError as error:
        raise ActionError(f"invalid arguments input: {error}") from error


def validate_action_arguments(arguments: list[str]) -> None:
    for argument in arguments:
        if argument == "--":
            break

        option = argument.split("=", 1)[0]
        if option in ADMINISTRATIVE_OPTIONS:
            raise ActionError(f"{option} is not supported by the GitHub Action")
        if option in INFORMATIONAL_OPTIONS:
            raise ActionError(f"{option} is not supported because it does not run an Arid scan")
        if option in UNSUPPORTED_ACTION_OPTIONS:
            raise ActionError(
                f"{option} is not supported because the GitHub Action does not supply stdin source"
            )


def has_option(arguments: list[str], name: str) -> bool:
    for argument in arguments:
        if argument == "--":
            break
        if argument.split("=", 1)[0] == name:
            return True
    return False


def build_command(
    paths: list[str],
    focus: list[str],
    arguments: list[str],
    report_path: Path,
    summary_path: Path | None = None,
    sarif_path: Path | None = None,
) -> list[str]:
    command = ["arid", *paths]

    for path in focus:
        command.extend(("--focus", path))

    command.extend(("--report", f"json={report_path}"))

    if summary_path is not None:
        command.extend(("--report", f"markdown={summary_path}"))

    if sarif_path is not None:
        command.extend(("--report", f"sarif={sarif_path}"))

    command.extend(arguments)
    return command


def _required_int(document: dict[str, object], name: str) -> int:
    value = document.get(name)
    if type(value) is not int or value < 0:
        raise ActionError(f"report field {name!r} must be a non-negative integer")
    return value


def read_report_document(path: Path) -> dict[str, object]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ActionError(f"failed to read report-v4 JSON: {error}") from error

    if not isinstance(document, dict):
        raise ActionError("report-v4 JSON must be an object")
    if document.get("schema_version") != 4:
        raise ActionError("action requires report schema_version 4")
    return document


def report_metrics(document: dict[str, object]) -> ReportMetrics:
    tool_version = document.get("tool_version")
    complete = document.get("complete")
    duplication_percent = document.get("duplication_percent")

    if not isinstance(tool_version, str) or not tool_version:
        raise ActionError("report field 'tool_version' must be a non-empty string")
    if type(complete) is not bool:
        raise ActionError("report field 'complete' must be a boolean")
    if type(duplication_percent) not in (int, float) or duplication_percent < 0:
        raise ActionError("report field 'duplication_percent' must be a non-negative number")

    return ReportMetrics(
        tool_version=tool_version,
        complete=complete,
        files=_required_int(document, "files"),
        duplicate_groups=_required_int(document, "duplicate_groups"),
        duplicate_lines=_required_int(document, "duplicate_lines"),
        duplication_percent=float(duplication_percent),
    )


def read_report(path: Path) -> ReportMetrics:
    return report_metrics(read_report_document(path))


def derive_summary(
    document: dict[str, object],
    *,
    ignore_files: bool,
    metrics: ReportMetrics | None = None,
) -> dict[str, object]:
    metrics = metrics or report_metrics(document)
    analysis = document.get("analysis")
    errors = document.get("errors")
    findings = document.get("findings")

    if not isinstance(analysis, dict):
        raise ActionError("report field 'analysis' must be an object")
    if not isinstance(errors, list):
        raise ActionError("report field 'errors' must be an array")
    if not isinstance(findings, list) or len(findings) != metrics.duplicate_groups:
        raise ActionError("report findings must match duplicate_groups")

    context = {"executable": 0, "declarative": 0, "mixed": 0}
    scope = {"function": 0, "module": 0, "class": 0, "mixed": 0}
    distribution = {"cross_file": 0, "same_file": 0, "hybrid": 0}
    distribution_keys = {
        "cross-file": "cross_file",
        "same-file": "same_file",
        "hybrid": "hybrid",
    }
    hotspot_counts: dict[str, list[int]] = {}
    occurrences = 0

    try:
        for finding in findings:
            if not isinstance(finding, dict):
                raise TypeError("finding is not an object")

            finding_context = finding["context"]
            finding_scope = finding["scope"]
            finding_distribution = finding["distribution"]
            if finding_context not in context:
                raise ValueError(f"unsupported context {finding_context!r}")
            if finding_scope not in scope:
                raise ValueError(f"unsupported scope {finding_scope!r}")
            distribution_key = distribution_keys.get(finding_distribution)
            if distribution_key is None:
                raise ValueError(f"unsupported distribution {finding_distribution!r}")

            context[finding_context] += 1
            scope[finding_scope] += 1
            distribution[distribution_key] += 1

            locations = finding["locations"]
            finding_occurrences = finding["occurrences"]
            if not isinstance(locations, list) or type(finding_occurrences) is not int:
                raise TypeError("finding occurrences or locations has invalid type")
            if finding_occurrences != len(locations):
                raise ValueError("finding occurrences does not match locations count")

            occurrences += finding_occurrences
            group_paths: set[str] = set()

            for location in locations:
                if not isinstance(location, dict):
                    raise TypeError("location is not an object")
                path = location["path"]
                if not isinstance(path, str) or not path:
                    raise ValueError("location path is empty or invalid")

                counts = hotspot_counts.setdefault(path, [0, 0])
                counts[1] += 1
                group_paths.add(path)

            for path in group_paths:
                hotspot_counts[path][0] += 1

        summary_analysis = {
            "min_lines": analysis["min_lines"],
            "ignore_comments": analysis["ignore_comments"],
            "ignore_docstrings": analysis["ignore_docstrings"],
            "ignore_imports": analysis["ignore_imports"],
            "ignore_signatures": analysis["ignore_signatures"],
            "same_file": analysis["same_file"],
            "hidden": analysis["hidden"],
            "ignore_files": ignore_files,
            "exclude": analysis["exclude"],
            "baseline_enabled": analysis["baseline_enabled"],
            "focus": analysis["focus"],
            "virtual_source": analysis["virtual_source"],
            "keep_going": analysis["keep_going"],
        }
    except (KeyError, TypeError, ValueError) as error:
        raise ActionError(f"invalid report-v4 summary data: {error}") from error

    hotspots = sorted(
        hotspot_counts.items(),
        key=lambda item: (-item[1][0], -item[1][1], item[0]),
    )[:5]

    return {
        "schema_version": 1,
        "tool_version": metrics.tool_version,
        "complete": metrics.complete,
        "analysis": summary_analysis,
        "errors": errors,
        "files": metrics.files,
        "files_with_duplicates": len(hotspot_counts),
        "source_lines": _required_int(document, "source_lines"),
        "analyzed_lines": _required_int(document, "analyzed_lines"),
        "duplicate_groups": metrics.duplicate_groups,
        "occurrences": occurrences,
        "duplicate_lines": metrics.duplicate_lines,
        "duplication_percent": metrics.duplication_percent,
        "context": context,
        "scope": scope,
        "distribution": distribution,
        "hotspots": [
            {"path": path, "groups": counts[0], "occurrences": counts[1]}
            for path, counts in hotspots
        ],
    }


def compact_summary_json(summary: dict[str, object]) -> str:
    return json.dumps(summary, ensure_ascii=False, separators=(",", ":"))


def should_fail(
    scan_exit_code: int,
    report: ReportMetrics | None,
    fail_on_findings: bool,
) -> bool:
    if scan_exit_code not in (0, 1, 2):
        raise ActionError(f"Arid returned unexpected exit code {scan_exit_code}")

    if scan_exit_code == 2:
        return True

    if report is None:
        raise ActionError("Arid completed without producing the required report-v4 JSON")
    if not report.complete:
        raise ActionError("Arid produced an incomplete report without exit code 2")
    if scan_exit_code == 1 and not report.has_findings:
        raise ActionError("Arid returned findings status without report findings")

    return report.has_findings and fail_on_findings


def sarif_ready(requested: bool, report: ReportMetrics | None, path: Path) -> bool:
    return requested and report is not None and report.complete and path.is_file()


def append_summary(source: Path, destination: Path) -> None:
    try:
        text = source.read_text(encoding="utf-8")
        with destination.open("a", encoding="utf-8", newline="\n") as output:
            output.write(text)
            if text and not text.endswith("\n"):
                output.write("\n")
    except OSError as error:
        raise ActionError(f"failed to append GitHub job summary: {error}") from error


def write_github_outputs(path: Path, values: dict[str, str]) -> None:
    try:
        with path.open("a", encoding="utf-8", newline="\n") as output:
            for name, value in values.items():
                if "\n" not in value and "\r" not in value:
                    output.write(f"{name}={value}\n")
                    continue

                delimiter = f"arid_{uuid.uuid4().hex}"
                output.write(f"{name}<<{delimiter}\n{value}\n{delimiter}\n")
    except OSError as error:
        raise ActionError(f"failed to write GitHub Action outputs: {error}") from error


def action_outputs(
    scan_exit_code: int,
    report: ReportMetrics | None,
    summary: dict[str, object] | None,
    fail_on_findings: bool,
    sarif_path: Path,
    sarif_requested: bool,
) -> dict[str, str]:
    values = {
        "tool-version": "",
        "has-findings": "",
        "duplicate-groups": "",
        "duplicate-lines": "",
        "duplication-percent": "",
        "files": "",
        "occurrences": "",
        "files-with-duplicates": "",
        "summary-json": "",
        "complete": "false",
        "scan-exit-code": str(scan_exit_code),
        "sarif-path": "",
        "sarif-ready": "false",
        "should-fail": str(should_fail(scan_exit_code, report, fail_on_findings)).lower(),
    }

    if report is not None:
        values.update(
            {
                "tool-version": report.tool_version,
                "has-findings": str(report.has_findings).lower(),
                "duplicate-groups": str(report.duplicate_groups),
                "duplicate-lines": str(report.duplicate_lines),
                "duplication-percent": str(report.duplication_percent),
                "files": str(report.files),
                "complete": str(report.complete).lower(),
            }
        )

    if summary is not None:
        values.update(
            {
                "occurrences": str(summary["occurrences"]),
                "files-with-duplicates": str(summary["files_with_duplicates"]),
                "summary-json": compact_summary_json(summary),
            }
        )
    elif report is not None:
        raise ActionError("report-v4 was available without a derived summary")

    if sarif_ready(sarif_requested, report, sarif_path):
        values["sarif-path"] = str(sarif_path)
        values["sarif-ready"] = "true"

    return values


def run_action() -> None:
    paths = split_lines(os.environ.get("ARID_ACTION_PATHS", "."), default=(".",))
    focus = split_lines(os.environ.get("ARID_ACTION_FOCUS", ""))
    arguments = parse_additional_arguments(os.environ.get("ARID_ACTION_ARGUMENTS", ""))
    validate_action_arguments(arguments)

    fail_on_findings = parse_bool(
        os.environ.get("ARID_ACTION_FAIL_ON_FINDINGS", "true"),
        "fail-on-findings",
    )
    sarif_requested = parse_bool(os.environ.get("ARID_ACTION_SARIF", "false"), "sarif")
    summary_requested = parse_bool(
        os.environ.get("ARID_ACTION_JOB_SUMMARY", "true"),
        "job-summary",
    )

    runner_temp_value = os.environ.get("RUNNER_TEMP")
    github_output_value = os.environ.get("GITHUB_OUTPUT")
    if not runner_temp_value:
        raise ActionError("RUNNER_TEMP is not set")
    if not github_output_value:
        raise ActionError("GITHUB_OUTPUT is not set")

    runner_temp = Path(runner_temp_value)
    work = Path(tempfile.mkdtemp(prefix="arid-action-", dir=runner_temp))
    report_path = work / "report.json"
    summary_path = work / "report.md" if summary_requested else None
    sarif_path = work / "report.sarif" if sarif_requested else work / "unused.sarif"

    command = build_command(
        paths,
        focus,
        arguments,
        report_path,
        summary_path=summary_path,
        sarif_path=sarif_path if sarif_requested else None,
    )
    scan_exit_code = subprocess.run(command, check=False).returncode

    report = None
    summary = None
    if report_path.is_file():
        document = read_report_document(report_path)
        report = report_metrics(document)
        summary = derive_summary(
            document,
            ignore_files=not has_option(arguments, "--no-ignore-files"),
            metrics=report,
        )

    if scan_exit_code in (0, 1) and report is None:
        raise ActionError("Arid completed without producing the required report-v4 JSON")

    if summary_requested and summary_path is not None and summary_path.is_file():
        summary_output = os.environ.get("GITHUB_STEP_SUMMARY")
        if not summary_output:
            raise ActionError("GITHUB_STEP_SUMMARY is not set")
        append_summary(summary_path, Path(summary_output))
    elif summary_requested and scan_exit_code in (0, 1):
        raise ActionError("Arid completed without producing the requested Markdown summary")

    if (
        sarif_requested
        and report is not None
        and report.complete
        and scan_exit_code in (0, 1)
        and not sarif_path.is_file()
    ):
        raise ActionError("Arid completed without producing the requested SARIF report")

    outputs = action_outputs(
        scan_exit_code,
        report,
        summary,
        fail_on_findings,
        sarif_path,
        sarif_requested,
    )
    write_github_outputs(Path(github_output_value), outputs)


def main() -> int:
    try:
        run_action()
    except ActionError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
