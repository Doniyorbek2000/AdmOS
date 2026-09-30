# ADR-0014: NUR Office foundation

- **Status:** RESEARCH REQUIRED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

NUR Writer, Sheets, and Slides need high-fidelity DOCX/XLSX/PPTX/ODF/PDF support. Writing an
office engine from scratch is not feasible. Candidate foundations carry different licenses,
trademark rules, and architectures ([OFFICE_STRATEGY.md](../OFFICE_STRATEGY.md)).

## Decision

No engine is chosen. **Nothing is forked, rebranded, or redistributed** until:

1. the licensing checklist in OFFICE_STRATEGY.md §5 is signed off by counsel;
2. a fidelity corpus comparison is completed;
3. a sandboxed integration spike (LibreOfficeKit-based viewer) demonstrates feasibility.

Leading research candidate: LibreOffice core via LibreOfficeKit with a native NUR UI. Comparison
candidate: ONLYOFFICE (requires commercial license consideration because of AGPL-3.0 and logo
terms). PDF: PDFium + qpdf.

## Consequences

Office work is limited to research, corpus building, and legal review until Phase 9.

## Alternatives considered

See OFFICE_STRATEGY.md §2.

## Revisit triggers

Legal review outcome; spike results; upstream license changes.
