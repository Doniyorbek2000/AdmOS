# ADR-0006: UI toolkit for first-party interfaces

- **Status:** RESEARCH REQUIRED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

Shell components and first-party apps need a toolkit with excellent accessibility (AT-SPI),
internationalisation (BiDi, input methods, complex scripts), HiDPI, performance on modest
hardware, and a sustainable license. The choice affects hiring, SDK, and the third-party ecosystem
for many years.

## Decision

No toolkit is chosen. A Phase 3 comparison builds the same two prototypes (a Settings panel with a
list/detail layout and the launcher) in each finalist and scores them.

### Candidates

| Toolkit | Language | License | Notes |
| --- | --- | --- | --- |
| GTK4 (via gtk-rs) | C core, Rust bindings | LGPL-2.1-or-later | Most mature Linux a11y and i18n; libadwaita carries GNOME design language (NUR would need its own styling/widgets) |
| Qt 6 (Widgets/QML) | C++ (Rust interop via cxx-qt) | LGPL-3.0 / GPL / commercial | Very mature, strong a11y; LGPL-3.0 obligations; C++/Rust boundary cost |
| iced | Rust | MIT | Used by COSMIC; accessibility via AccessKit still maturing — must be verified |
| Slint | Rust (+ C++/JS) | GPL-3.0 / royalty-free / commercial | Lightweight, declarative; license terms need review for NUR use |

### Criteria (weighted)

Accessibility (screen reader, keyboard) 25 %; i18n/BiDi/IME 15 %; performance/memory 15 %;
Rust integration 10 %; license 10 %; maturity/maintenance 10 %; theming to NUR design system 10 %;
developer experience 5 %.

## Consequences

Until decided, no production UI code is written; design tokens are kept toolkit-independent.

## Alternatives considered

Web-based UI (Electron/WebView) for core utilities — rejected by
[TECH_STACK.md](../TECH_STACK.md) non-choices.

## Revisit triggers

Phase 3 comparison completes.
