"""Unit tests for tools/check_repo.py (run: python3 -m unittest discover -s tools/tests -v)."""

from __future__ import annotations

import io
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import check_repo  # noqa: E402  (import after sys.path adjustment)

VALID_ADR = """# ADR-{num}: Example decision

- **Status:** {status}
- **Date:** 2026-09-30
- **Deciders:** Test

## Context

Text.

## Decision

Text.

## Consequences

Text.
"""


def write(root: Path, relative: str, content: str) -> Path:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


def adr_index(rows: list[tuple[str, str, str]]) -> str:
    lines = ["# ADRs", "", "| ADR | Title | Status |", "| --- | --- | --- |"]
    lines += [f"| [{num}]({target}) | Title | {status} |" for num, target, status in rows]
    return "\n".join(lines) + "\n"


class RepoTestCase(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def checks(self, name: str) -> list[check_repo.Finding]:
        return check_repo.run_checks(self.root, [name])

    def make_valid_adrs(self, statuses: list[str]) -> None:
        rows = []
        for number, status in enumerate(statuses):
            num = f"{number:04d}"
            target = f"{num}-example-{number}.md"
            write(self.root, f"docs/adr/{target}", VALID_ADR.format(num=num, status=status))
            rows.append((num, target, status))
        write(self.root, "docs/adr/README.md", adr_index(rows))


class RequiredFilesTests(RepoTestCase):
    def test_reports_every_missing_file(self) -> None:
        findings = self.checks("required-files")
        self.assertEqual(len(findings), len(check_repo.REQUIRED_FILES))

    def test_passes_when_all_present(self) -> None:
        for name in check_repo.REQUIRED_FILES:
            write(self.root, name, "# x\n")
        self.assertEqual(self.checks("required-files"), [])


class AdrTests(RepoTestCase):
    def test_valid_adrs_pass(self) -> None:
        self.make_valid_adrs(["DECIDED", "PROPOSED", "RESEARCH REQUIRED"])
        self.assertEqual(self.checks("adr"), [])

    def test_invalid_status_is_reported(self) -> None:
        self.make_valid_adrs(["DECIDED"])
        write(self.root, "docs/adr/0000-example-0.md", VALID_ADR.format(num="0000", status="MAYBE"))
        messages = [f.message for f in self.checks("adr")]
        self.assertTrue(any("invalid status 'MAYBE'" in m for m in messages), messages)

    def test_gap_in_numbering_is_reported(self) -> None:
        self.make_valid_adrs(["DECIDED", "DECIDED", "DECIDED"])
        (self.root / "docs/adr/0001-example-1.md").unlink()
        write(
            self.root,
            "docs/adr/README.md",
            adr_index([("0000", "0000-example-0.md", "DECIDED"), ("0002", "0002-example-2.md", "DECIDED")]),
        )
        messages = [f.message for f in self.checks("adr")]
        self.assertTrue(any("missing: 0001" in m for m in messages), messages)

    def test_duplicate_number_is_reported(self) -> None:
        self.make_valid_adrs(["DECIDED"])
        write(self.root, "docs/adr/0000-other.md", VALID_ADR.format(num="0000", status="DECIDED"))
        messages = [f.message for f in self.checks("adr")]
        self.assertTrue(any("used more than once" in m for m in messages), messages)

    def test_title_number_mismatch_is_reported(self) -> None:
        self.make_valid_adrs(["DECIDED"])
        write(self.root, "docs/adr/0000-example-0.md", VALID_ADR.format(num="0007", status="DECIDED"))
        messages = [f.message for f in self.checks("adr")]
        self.assertTrue(any("does not match file number" in m for m in messages), messages)

    def test_missing_section_and_field_are_reported(self) -> None:
        self.make_valid_adrs(["DECIDED"])
        broken = VALID_ADR.format(num="0000", status="DECIDED")
        broken = broken.replace("- **Deciders:** Test\n", "").replace("## Consequences", "## Notes")
        write(self.root, "docs/adr/0000-example-0.md", broken)
        messages = [f.message for f in self.checks("adr")]
        self.assertIn("missing metadata field 'Deciders'", messages)
        self.assertIn("missing section '## Consequences'", messages)

    def test_bad_filename_is_reported(self) -> None:
        self.make_valid_adrs(["DECIDED"])
        write(self.root, "docs/adr/1-Bad_Name.md", VALID_ADR.format(num="0001", status="DECIDED"))
        messages = [f.message for f in self.checks("adr")]
        self.assertTrue(any("file name must match" in m for m in messages), messages)

    def test_unindexed_adr_and_status_mismatch_are_reported(self) -> None:
        self.make_valid_adrs(["DECIDED", "PROPOSED"])
        write(self.root, "docs/adr/README.md", adr_index([("0000", "0000-example-0.md", "PROPOSED")]))
        messages = [f.message for f in self.checks("adr")]
        self.assertTrue(any("ADR 0001 is not listed" in m for m in messages), messages)
        self.assertTrue(any("'DECIDED' in the record but 'PROPOSED'" in m for m in messages), messages)

    def test_index_row_without_file_is_reported(self) -> None:
        self.make_valid_adrs(["DECIDED"])
        write(
            self.root,
            "docs/adr/README.md",
            adr_index([("0000", "0000-example-0.md", "DECIDED"), ("0001", "0001-ghost.md", "DECIDED")]),
        )
        messages = [f.message for f in self.checks("adr")]
        self.assertTrue(any("no such ADR file exists" in m for m in messages), messages)


class SlugTests(unittest.TestCase):
    def test_github_slug_rules(self) -> None:
        cases = {
            "2. Trust boundaries": "2-trust-boundaries",
            "7. NUR OS Developer Preview 0.1 — definition": "7-nur-os-developer-preview-01--definition",
            "Phase 1 — Bootable NUR Base": "phase-1--bootable-nur-base",
            "`nur-updated` design": "nur-updated-design",
            "See [link](x.md) here": "see-link-here",
            "Permission model (summary)": "permission-model-summary",
        }
        for heading, expected in cases.items():
            with self.subTest(heading=heading):
                self.assertEqual(check_repo.github_slug(heading), expected)

    def test_duplicate_headings_get_suffixes_and_code_is_ignored(self) -> None:
        text = "# A\n\n```\n# Not a heading\n```\n\n## A\n"
        self.assertEqual(check_repo.heading_anchors(text), {"a", "a-1"})


class LinkTests(RepoTestCase):
    def test_valid_file_and_anchor_links_pass(self) -> None:
        write(self.root, "docs/B.md", "# Title\n\n## Section two\n")
        write(
            self.root,
            "docs/A.md",
            "# A\n\n[b](B.md) [s](B.md#section-two) [self](#a) [ext](https://example.org)\n",
        )
        self.assertEqual(self.checks("links"), [])

    def test_broken_file_link_is_reported(self) -> None:
        write(self.root, "A.md", "[x](missing.md)\n")
        findings = self.checks("links")
        self.assertEqual(len(findings), 1)
        self.assertIn("broken link 'missing.md'", findings[0].message)
        self.assertEqual(findings[0].line, 1)

    def test_broken_anchor_is_reported(self) -> None:
        write(self.root, "B.md", "# Title\n")
        write(self.root, "A.md", "[x](B.md#nope)\n")
        messages = [f.message for f in self.checks("links")]
        self.assertEqual(messages, ["anchor '#nope' not found in 'B.md'"])

    def test_links_in_code_are_ignored(self) -> None:
        write(self.root, "A.md", "`[x](missing.md)`\n\n```\n[y](missing.md)\n```\n")
        self.assertEqual(self.checks("links"), [])

    def test_link_escaping_repository_is_reported(self) -> None:
        write(self.root, "A.md", "[x](../../etc/passwd)\n")
        messages = [f.message for f in self.checks("links")]
        self.assertTrue(any("leaves the repository" in m for m in messages), messages)


class SecretTests(RepoTestCase):
    def test_detects_known_secret_formats(self) -> None:
        samples = {
            "private key block": "-----" + "BEGIN OPENSSH PRIVATE KEY-----",
            "AWS access key id": "AKIA" + "ABCDEFGHIJKLMNOP",
            "GitHub token": "ghp_" + "a" * 36,
            "Anthropic API key": "sk-ant-" + "b" * 30,
        }
        for label, secret in samples.items():
            with self.subTest(label=label):
                write(self.root, "leak.txt", f"value = {secret}\n")
                messages = [f.message for f in self.checks("secrets")]
                self.assertIn(f"possible {label} committed", messages)

    def test_clean_file_passes(self) -> None:
        write(self.root, "ok.md", "Never commit private keys. Use the keyring.\n")
        self.assertEqual(self.checks("secrets"), [])

    def test_skips_git_directory(self) -> None:
        write(self.root, ".git/config", "AKIA" + "ABCDEFGHIJKLMNOP\n")
        self.assertEqual(self.checks("secrets"), [])


class WhitespaceTests(RepoTestCase):
    def test_clean_file_passes(self) -> None:
        write(self.root, "a.md", "line\n")
        self.assertEqual(self.checks("whitespace"), [])

    def test_reports_trailing_whitespace_missing_newline_crlf_and_blank_tail(self) -> None:
        write(self.root, "a.md", "line \n")
        (self.root / "b.md").write_bytes(b"line")
        (self.root / "c.md").write_bytes(b"line\r\n")
        write(self.root, "d.md", "line\n\n")
        findings = {(f.path, f.message) for f in self.checks("whitespace")}
        self.assertIn(("a.md", "trailing whitespace"), findings)
        self.assertIn(("b.md", "missing final newline"), findings)
        self.assertIn(("c.md", "CRLF line endings (use LF)"), findings)
        self.assertIn(("d.md", "multiple trailing blank lines"), findings)

    def test_non_text_files_are_ignored(self) -> None:
        (self.root / "image.png").write_bytes(b"\x89PNG \n\n")
        self.assertEqual(self.checks("whitespace"), [])


class MainTests(RepoTestCase):
    def test_exit_codes(self) -> None:
        write(self.root, "A.md", "[x](missing.md)\n")
        with redirect_stdout(io.StringIO()) as out:
            self.assertEqual(check_repo.main(["--root", str(self.root), "--check", "links"]), 1)
        self.assertIn("FAILED: 1 problem(s)", out.getvalue())

        write(self.root, "A.md", "fine\n")
        with redirect_stdout(io.StringIO()) as out:
            self.assertEqual(check_repo.main(["--root", str(self.root), "--check", "links"]), 0)
        self.assertIn("OK", out.getvalue())

    def test_real_repository_passes(self) -> None:
        repo_root = Path(__file__).resolve().parent.parent.parent
        findings = check_repo.run_checks(repo_root)
        self.assertEqual([f.render() for f in findings], [])


if __name__ == "__main__":
    unittest.main()
