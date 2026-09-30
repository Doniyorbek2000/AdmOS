# NUR Office — Strategy and Foundation Research

Status: **RESEARCH REQUIRED** ([ADR-0014](adr/0014-office-suite-foundation.md)). No foundation has
been selected, forked, rebranded, or redistributed. **Nothing may be rebranded or redistributed
until the licensing review in §5 is signed off by legal counsel.**

## 1. Goals

- NUR Writer (word processing), NUR Sheets (spreadsheets), NUR Slides (presentations), PDF tools.
- High-fidelity import/export: **DOCX, XLSX, PPTX, PDF, ODT/ODS/ODP**.
- Native NUR look, performance, accessibility, and localisation (uz-Latn, en, ru and beyond).
- Safe integration with NUR AI via documented intents ([AI_SECURITY.md](AI_SECURITY.md)).
- Long-term independence: NUR must be able to maintain the product even if an upstream changes
  direction or license.

Non-goal: writing an office engine from scratch. Decades of layout, formula, and file-format
compatibility work cannot be reproduced quickly.

## 2. Candidate foundations

License information must be re-verified against the exact version adopted.

| Candidate | License (as understood) | Architecture | OOXML fidelity | Embeddability | Maintenance | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| **LibreOffice core** (via LibreOfficeKit) | MPL-2.0 (portions also LGPL-3.0-or-later); trademarks owned by The Document Foundation | C++ monolith; internal model is ODF-oriented; LibreOfficeKit exposes tile rendering + UNO API | Good and improving; complex DOCX/PPTX layouts can differ | LibreOfficeKit powers Collabora Online and several mobile/desktop front-ends | Very active; large community + commercial ecosystem (Collabora, allotropia) | File-level copyleft (MPL) is compatible with a proprietary NUR UI shell if MPL files' modifications are published |
| **ONLYOFFICE** (Document Server / Desktop Editors) | AGPL-3.0 with additional Section 7 terms (logo retention; no trademark grant); commercial license from vendor | Editors in JavaScript (canvas) + C++ core; OOXML is the native model | Very strong for OOXML | Desktop Editors are a CEF/Chromium-based app | Active, single-vendor controlled | AGPL + mandatory logo retention conflicts with an independent NUR brand unless a commercial license is purchased; Chromium-based UI conflicts with the "no heavy runtime" principle |
| **Collabora Online / CODE** | MPL-2.0 | LibreOffice core + web front-end | Same as LibreOffice | Web/server model | Active | Relevant for NUR Cloud collaborative editing later |
| Calligra Suite | GPL-2.0-or-later / LGPL | Qt/KDE | Weak for complex OOXML | Low | Small community | Not recommended |
| Apache OpenOffice | Apache-2.0 | C++ monolith | Behind LibreOffice | Low | Low activity | Permissive but not a viable long-term base |
| Proprietary engine licensing (commercial SDKs) | Commercial | Varies | Often excellent | Varies | Vendor | Cost and dependence; conflicts with independence goal |

### PDF components

| Candidate | License | Use |
| --- | --- | --- |
| PDFium | BSD-3-Clause (with Apache-2.0 portions) | Rendering, forms — permissive, used by Chromium |
| MuPDF | AGPL-3.0 or commercial | Excellent rendering/annotation; AGPL requires legal review |
| Poppler | GPL-2.0-or-later / GPL-3.0 | Mature rendering; GPL linking constraints |
| PDF.js | Apache-2.0 | Web-based viewer |
| qpdf | Apache-2.0 | Structural manipulation (merge, split, encrypt) |

## 3. Recommended research direction (PROPOSED for Phase 9 spike, pre-reviewed in Phase 7)

1. **Primary candidate: LibreOffice core via LibreOfficeKit**, with a native NUR front-end
   (toolbar/sidebar/dialogs in the NUR toolkit, document canvas rendered by LOKit tiles).
   Rationale: permissive-enough file-level copyleft, strong ODF + good OOXML, huge format
   coverage, multiple independent commercial support vendors (reduces single-vendor risk).
2. **Comparison candidate: ONLYOFFICE**, evaluated for OOXML fidelity only; adoption would require a
   commercial license negotiation because of AGPL + logo terms.
3. **PDF**: PDFium for viewing/forms + qpdf for manipulation (both permissive).
4. Measure with a **fidelity corpus**: a curated set of real-world DOCX/XLSX/PPTX files (including
   Uzbek and Russian text, RTL samples, complex tables, charts, pivot tables, macros-disabled
   VBA files, tracked changes, comments). Score round-trip fidelity by rendering both original
   (reference renderer) and round-tripped files to images and computing visual diffs, plus
   structural checks (formula results, styles, comments preserved).

## 4. Integration architecture

```text
NUR Writer / Sheets / Slides (NUR UI, sandboxed Flatpak apps)
        │  org.nur.Documents1 (document service API; toolkit-independent)
        ▼
nur-docs-engine (per-document sandboxed worker process; no network; file access via portal fds)
        │
        ▼
Engine adapter (LibreOfficeKit | alternative) — the only engine-specific code
```

- **Per-document worker processes**: a malicious file that exploits the engine is confined to a
  process with no network and access to that single document fd.
- **Macros disabled by default**; enabling requires a per-document trusted prompt and runs in the
  same sandbox. VBA execution is out of scope.
- The document service API exposes AI-safe intents (analyse, summarise, create) that operate on
  structured data, so the AI never needs raw engine access.
- Engine swap = new adapter; apps and AI integration are unaffected.

## 5. Licensing review checklist (must be completed before any redistribution)

| Question | Owner |
| --- | --- |
| Which files/modules are MPL-2.0 vs LGPL-3.0; does our build combine them in a way that changes obligations? | Legal + engineering |
| Obligations for distributing modified MPL files (source availability, notices) and how our release pipeline fulfils them | Legal + release |
| Trademark: remove all "LibreOffice"/"The Document Foundation" branding; confirm permitted descriptive references ("based on LibreOffice technology") | Legal |
| Bundled third-party libraries inside the engine (fonts, dictionaries, hyphenation patterns, ICU, etc.) and their licenses | Engineering (ScanCode) |
| Dictionaries/thesauri for uz, ru, en: licenses (many are GPL/LGPL/MPL/CC) | Engineering + legal |
| Patent considerations for file formats and codecs embedded in documents | Legal |
| If ONLYOFFICE is considered: commercial license cost and terms; AGPL network clause impact on NUR Cloud | Legal + business |
| Export-control and localisation of spell-check data | Legal |

## 6. Phasing

- Phase 7: licensing review + fidelity corpus creation + technical spike (read-only viewer via
  LOKit in a sandbox) — **no public distribution**.
- Phase 9: NUR Writer/Sheets/Slides MVP on the chosen engine.
- Post-1.0: collaborative editing (Collabora-style) as part of NUR Cloud, subject to privacy design.
