#!/usr/bin/env python3
"""Repository validation for NUR OS.

Checks (standard library only, Python 3.11+):
  * required foundation files exist;
  * ADR files follow naming, title, and metadata conventions, are numbered contiguously,
    and are listed in docs/adr/README.md with a matching status;
  * relative Markdown links resolve to existing files and, when present, to existing
    heading anchors (GitHub slug rules);
  * no obvious secrets (private keys, well-known token formats) are committed;
  * text files have no trailing whitespace and end with exactly one newline.

Exit status: 0 when no errors were found, 1 otherwise.
"""

from __future__ import annotations

import argparse
import re
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator

REQUIRED_FILES: tuple[str, ...] = (
    "README.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "AGENTS.md",
    "docs/VISION.md",
    "docs/ARCHITECTURE.md",
    "docs/SECURITY_MODEL.md",
    "docs/THREAT_MODEL.md",
    "docs/ROADMAP.md",
    "docs/TECH_STACK.md",
    "docs/REPOSITORY_STRATEGY.md",
    "docs/UPDATE_ARCHITECTURE.md",
    "docs/APP_PLATFORM.md",
    "docs/DESKTOP_ARCHITECTURE.md",
    "docs/HARDWARE_COMPATIBILITY.md",
    "docs/AI_SECURITY.md",
    "docs/OFFICE_STRATEGY.md",
    "docs/TESTING_STRATEGY.md",
    "docs/RELEASE_STRATEGY.md",
    "docs/LICENSING_POLICY.md",
    "docs/RISKS_AND_DECISIONS.md",
    "docs/adr/README.md",
    "docs/adr/template.md",
)

ADR_DIR = Path("docs/adr")
ADR_INDEX = ADR_DIR / "README.md"
ADR_NON_RECORDS = frozenset({"README.md", "template.md"})
ADR_FILENAME_RE = re.compile(r"^(?P<num>\d{4})-(?P<slug>[a-z0-9]+(?:-[a-z0-9]+)*)\.md$")
ADR_TITLE_RE = re.compile(r"^# ADR-(?P<num>\d{4}): \S")
ADR_FIELD_RE = re.compile(r"^- \*\*(?P<key>[A-Za-z ]+):\*\* (?P<value>.+)$")
ADR_INDEX_ROW_RE = re.compile(
    r"^\|\s*\[(?P<num>\d{4})\]\((?P<target>[^)]+)\)\s*\|[^|]*\|\s*(?P<status>[A-Z ]+?)\s*\|\s*$"
)
ADR_STATUSES = frozenset({"DECIDED", "PROPOSED", "RESEARCH REQUIRED", "SUPERSEDED", "REJECTED"})
ADR_REQUIRED_FIELDS = ("Status", "Date", "Deciders")
ADR_REQUIRED_SECTIONS = ("## Context", "## Decision", "## Consequences")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

TEXT_SUFFIXES = frozenset(
    {".md", ".py", ".yml", ".yaml", ".toml", ".txt", ".json", ".sh", ".rs", ".cfg", ".conf"}
)
TEXT_FILENAMES = frozenset({".editorconfig", ".gitattributes", ".gitignore"})
SKIP_DIRS = frozenset({".git", "target", "build", "out", "__pycache__", ".venv", "node_modules"})

# Patterns are assembled from fragments so this file never matches itself.
_BEGIN = "-----" + "BEGIN"
SECRET_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("private key block", re.compile(_BEGIN + r" (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----")),
    ("AWS access key id", re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")),
    ("GitHub token", re.compile(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b")),
    ("GitHub fine-grained token", re.compile(r"\bgithub_pat_[A-Za-z0-9_]{60,}\b")),
    ("Slack token", re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b")),
    ("Google API key", re.compile(r"\bAIza[0-9A-Za-z_\-]{35}\b")),
    ("Anthropic API key", re.compile(r"\bsk-ant-[A-Za-z0-9_\-]{20,}")),
    ("OpenAI-style API key", re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9]{32,}\b")),
)

MARKDOWN_LINK_RE = re.compile(r"(?<!!)\[(?P<text>[^\]]*)\]\((?P<target>[^)\s]+)(?:\s+\"[^\"]*\")?\)")
INLINE_CODE_RE = re.compile(r"`[^`]*`")
FENCE_RE = re.compile(r"^\s*(```|~~~)")
HEADING_RE = re.compile(r"^(?P<hashes>#{1,6})\s+(?P<text>.+?)\s*#*\s*$")
EXTERNAL_LINK_PREFIXES = ("http://", "https://", "mailto:", "tel:")


@dataclass(frozen=True, order=True)
class Finding:
    """A single validation error, anchored to a file and optional line."""

    path: str
    line: int
    check: str
    message: str

    def render(self) -> str:
        location = f"{self.path}:{self.line}" if self.line else self.path
        return f"{location}: [{self.check}] {self.message}"


def iter_repo_files(root: Path) -> Iterator[Path]:
    """Yield all files under root, skipping VCS metadata and build output."""
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        relative_parts = path.relative_to(root).parts
        if any(part in SKIP_DIRS for part in relative_parts[:-1]):
            continue
        yield path


def is_text_file(path: Path) -> bool:
    return path.suffix in TEXT_SUFFIXES or path.name in TEXT_FILENAMES


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def rel(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix()


# --------------------------------------------------------------------------- required files


def check_required_files(root: Path) -> list[Finding]:
    return [
        Finding(name, 0, "required-files", "required file is missing")
        for name in REQUIRED_FILES
        if not (root / name).is_file()
    ]


# --------------------------------------------------------------------------- ADRs


def parse_adr_fields(lines: Iterable[str]) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in lines:
        match = ADR_FIELD_RE.match(line)
        if match:
            fields.setdefault(match.group("key").strip(), match.group("value").strip())
    return fields


def check_adrs(root: Path) -> list[Finding]:
    adr_dir = root / ADR_DIR
    if not adr_dir.is_dir():
        return [Finding(ADR_DIR.as_posix(), 0, "adr", "ADR directory is missing")]

    findings: list[Finding] = []
    statuses: dict[str, str] = {}
    numbers: list[int] = []

    for path in sorted(adr_dir.glob("*.md")):
        if path.name in ADR_NON_RECORDS:
            continue
        path_str = rel(root, path)
        name_match = ADR_FILENAME_RE.match(path.name)
        if not name_match:
            findings.append(
                Finding(path_str, 0, "adr", "file name must match NNNN-lowercase-slug.md")
            )
            continue
        number = name_match.group("num")
        numbers.append(int(number))

        lines = read_text(path).splitlines()
        title_match = ADR_TITLE_RE.match(lines[0]) if lines else None
        if not title_match:
            findings.append(Finding(path_str, 1, "adr", "first line must be '# ADR-NNNN: Title'"))
        elif title_match.group("num") != number:
            findings.append(
                Finding(path_str, 1, "adr", f"title number ADR-{title_match.group('num')} "
                        f"does not match file number {number}")
            )

        fields = parse_adr_fields(lines)
        for key in ADR_REQUIRED_FIELDS:
            if key not in fields:
                findings.append(Finding(path_str, 0, "adr", f"missing metadata field '{key}'"))
        status = fields.get("Status")
        if status is not None:
            if status not in ADR_STATUSES:
                allowed = ", ".join(sorted(ADR_STATUSES))
                findings.append(
                    Finding(path_str, 0, "adr", f"invalid status '{status}' (allowed: {allowed})")
                )
            statuses[number] = status
        date = fields.get("Date")
        if date is not None and not DATE_RE.match(date):
            findings.append(Finding(path_str, 0, "adr", f"date '{date}' is not YYYY-MM-DD"))

        for section in ADR_REQUIRED_SECTIONS:
            if not any(line.strip() == section or line.startswith(section + " ") for line in lines):
                findings.append(Finding(path_str, 0, "adr", f"missing section '{section}'"))

    duplicates = [n for n, count in Counter(numbers).items() if count > 1]
    for duplicate in sorted(duplicates):
        findings.append(
            Finding(ADR_DIR.as_posix(), 0, "adr", f"ADR number {duplicate:04d} is used more than once")
        )
    unique_numbers = sorted(set(numbers))
    expected = list(range(len(unique_numbers)))
    if unique_numbers != expected:
        missing = sorted(set(expected) - set(unique_numbers))
        findings.append(
            Finding(ADR_DIR.as_posix(), 0, "adr",
                    "ADR numbers must be contiguous from 0000; missing: "
                    + ", ".join(f"{n:04d}" for n in missing))
        )

    findings.extend(check_adr_index(root, statuses))
    return findings


def check_adr_index(root: Path, statuses: dict[str, str]) -> list[Finding]:
    index_path = root / ADR_INDEX
    if not index_path.is_file():
        return [Finding(ADR_INDEX.as_posix(), 0, "adr-index", "ADR index is missing")]

    findings: list[Finding] = []
    indexed: dict[str, str] = {}
    for line_number, line in enumerate(read_text(index_path).splitlines(), start=1):
        match = ADR_INDEX_ROW_RE.match(line)
        if not match:
            continue
        number = match.group("num")
        target = match.group("target")
        if not target.startswith(f"{number}-"):
            findings.append(
                Finding(ADR_INDEX.as_posix(), line_number, "adr-index",
                        f"row {number} links to '{target}', which has a different number")
            )
        indexed[number] = match.group("status").strip()
        if number in statuses and indexed[number] != statuses[number]:
            findings.append(
                Finding(ADR_INDEX.as_posix(), line_number, "adr-index",
                        f"ADR {number} status is '{statuses[number]}' in the record "
                        f"but '{indexed[number]}' in the index")
            )

    for number in sorted(set(statuses) - set(indexed)):
        findings.append(
            Finding(ADR_INDEX.as_posix(), 0, "adr-index", f"ADR {number} is not listed in the index")
        )
    for number in sorted(set(indexed) - set(statuses)):
        findings.append(
            Finding(ADR_INDEX.as_posix(), 0, "adr-index",
                    f"index lists ADR {number}, but no such ADR file exists")
        )
    return findings


# --------------------------------------------------------------------------- Markdown links


def github_slug(heading: str) -> str:
    """Approximate GitHub's heading anchor algorithm."""
    text = INLINE_CODE_RE.sub(lambda m: m.group(0).strip("`"), heading)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)  # keep link text only
    text = text.strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def markdown_lines_outside_code(text: str) -> Iterator[tuple[int, str]]:
    """Yield (line_number, line) for lines outside fenced code blocks."""
    in_fence = False
    fence_marker = ""
    for line_number, line in enumerate(text.splitlines(), start=1):
        fence = FENCE_RE.match(line)
        if fence:
            marker = fence.group(1)
            if not in_fence:
                in_fence, fence_marker = True, marker
                continue
            if marker == fence_marker:
                in_fence = False
                continue
        if not in_fence:
            yield line_number, line


def heading_anchors(text: str) -> set[str]:
    anchors: set[str] = set()
    seen: Counter[str] = Counter()
    for _, line in markdown_lines_outside_code(text):
        match = HEADING_RE.match(line)
        if not match:
            continue
        slug = github_slug(match.group("text"))
        anchor = slug if seen[slug] == 0 else f"{slug}-{seen[slug]}"
        seen[slug] += 1
        anchors.add(anchor)
    return anchors


def check_markdown_links(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    anchor_cache: dict[Path, set[str]] = {}

    def anchors_for(path: Path) -> set[str]:
        if path not in anchor_cache:
            anchor_cache[path] = heading_anchors(read_text(path))
        return anchor_cache[path]

    for path in iter_repo_files(root):
        if path.suffix != ".md":
            continue
        path_str = rel(root, path)
        for line_number, line in markdown_lines_outside_code(read_text(path)):
            for match in MARKDOWN_LINK_RE.finditer(INLINE_CODE_RE.sub("", line)):
                target = match.group("target")
                if target.startswith(EXTERNAL_LINK_PREFIXES):
                    continue
                file_part, _, anchor = target.partition("#")
                resolved = path if not file_part else (path.parent / file_part).resolve()
                try:
                    resolved.relative_to(root.resolve())
                except ValueError:
                    findings.append(
                        Finding(path_str, line_number, "links", f"link '{target}' leaves the repository")
                    )
                    continue
                if not resolved.exists():
                    findings.append(
                        Finding(path_str, line_number, "links", f"broken link '{target}'")
                    )
                    continue
                if anchor and resolved.is_file() and resolved.suffix == ".md":
                    if anchor not in anchors_for(resolved):
                        findings.append(
                            Finding(path_str, line_number, "links",
                                    f"anchor '#{anchor}' not found in '{rel(root, resolved)}'")
                        )
    return findings


# --------------------------------------------------------------------------- secrets


def check_secrets(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    for path in iter_repo_files(root):
        try:
            text = read_text(path)
        except (UnicodeDecodeError, OSError):
            continue  # binary or unreadable files are out of scope for the text scanner
        for line_number, line in enumerate(text.splitlines(), start=1):
            for label, pattern in SECRET_PATTERNS:
                if pattern.search(line):
                    findings.append(
                        Finding(rel(root, path), line_number, "secrets", f"possible {label} committed")
                    )
    return findings


# --------------------------------------------------------------------------- whitespace


def check_whitespace(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    for path in iter_repo_files(root):
        if not is_text_file(path):
            continue
        path_str = rel(root, path)
        raw = path.read_bytes()
        if not raw:
            continue
        if b"\r\n" in raw:
            findings.append(Finding(path_str, 0, "whitespace", "CRLF line endings (use LF)"))
        if not raw.endswith(b"\n"):
            findings.append(Finding(path_str, 0, "whitespace", "missing final newline"))
        elif raw.endswith(b"\n\n"):
            findings.append(Finding(path_str, 0, "whitespace", "multiple trailing blank lines"))
        for line_number, line in enumerate(raw.decode("utf-8", "replace").splitlines(), start=1):
            if line != line.rstrip(" \t"):
                findings.append(Finding(path_str, line_number, "whitespace", "trailing whitespace"))
    return findings


# --------------------------------------------------------------------------- driver

CHECKS = {
    "required-files": check_required_files,
    "adr": check_adrs,
    "links": check_markdown_links,
    "secrets": check_secrets,
    "whitespace": check_whitespace,
}


def run_checks(root: Path, selected: Iterable[str] | None = None) -> list[Finding]:
    names = list(selected) if selected else list(CHECKS)
    findings: list[Finding] = []
    for name in names:
        findings.extend(CHECKS[name](root))
    return sorted(findings)


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate the NUR OS repository foundation.")
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="repository root (default: parent of tools/)",
    )
    parser.add_argument(
        "--check",
        action="append",
        choices=sorted(CHECKS),
        help="run only the named check (repeatable)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    root: Path = args.root.resolve()
    if not root.is_dir():
        sys.stderr.write(f"error: root '{root}' is not a directory\n")
        return 2
    findings = run_checks(root, args.check)
    for finding in findings:
        sys.stdout.write(finding.render() + "\n")
    checks_run = ", ".join(args.check or CHECKS)
    if findings:
        sys.stdout.write(f"\nFAILED: {len(findings)} problem(s) found (checks: {checks_run})\n")
        return 1
    sys.stdout.write(f"OK: no problems found (checks: {checks_run})\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
