# NUR OS — Vision

## 1. Mission

Build a fast, stable, secure-by-default desktop operating system with a coherent user experience
for ordinary and professional users, on hardware people already own — and a platform that
developers, organisations, and first-party services (Store, AI, Office, Cloud) can build on for
decades.

## 2. What NUR OS is

NUR OS is a **platform**, not a theme or a package collection:

- a Linux LTS kernel and the Linux driver ecosystem as the hardware foundation;
- an **image-based, verified, atomically updated** base system (NUR Core);
- a **security architecture** designed in from day one (NUR Security);
- **stable, versioned platform APIs** that applications and the desktop consume, so that major
  components can be replaced without rewriting the ecosystem;
- **NUR Desktop**: a Wayland-native desktop with its own identity;
- first-party applications: Settings, Files, Terminal, Software/Store, Update, Security,
  Accounts, Backup/Recovery;
- later: NUR AI, NUR Office (Writer, Sheets, Slides, PDF), SDK, developer platform, and optional
  cloud/sync.

## 3. What NUR OS is not

- Not a new kernel or a from-scratch driver stack in its first generation.
- Not a reskin of an existing distribution or desktop environment.
- Not a pixel copy of Windows or macOS. We study their strengths and design independently.
  No proprietary code, assets, or branding from other vendors is used.
- Not a promise of universal hardware or Windows-application compatibility. Compatibility is
  measured and published, never assumed.
- Not a platform that monetises user data. Telemetry is opt-in, documented, and inspectable.

## 4. Engineering priorities (in order)

When goals conflict, the higher item wins. Deviations need an ADR.

1. Stability
2. Security
3. Data integrity
4. Performance
5. Hardware compatibility
6. User experience
7. Maintainability
8. Accessibility
9. Battery efficiency
10. Developer experience

Security and data integrity are never traded for shipping speed. Accessibility is ranked eighth
for trade-off purposes only; it is a release gate for every user-facing component
([TESTING_STRATEGY.md](TESTING_STRATEGY.md)).

## 5. Target users

| Segment | Needs | Implications |
| --- | --- | --- |
| Everyday users | It just works; safe by default; understandable settings; no terminal required | Guided installer, automatic updates with rollback, clear permission prompts |
| Professionals / knowledge workers | Reliability, office compatibility, multi-monitor, performance | DOCX/XLSX/PPTX fidelity, HiDPI/fractional scaling, fast search |
| Developers | Terminal, containers, toolchains, SDK | Mutable dev containers isolated from the immutable host |
| Organisations (later) | Fleet management, policy, audit, compliance | Policy engine, audit logs, enterprise update channels |
| Education / public sector (later) | Low-cost hardware, localisation (Uzbek, Russian, English and beyond) | Modest-hardware performance budgets, full i18n |

## 6. Product pillars

1. **Trustworthy** — verified boot chain, signed everything, least privilege, permissions the
   user can understand and revoke, AI that cannot act without authority.
2. **Resilient** — atomic updates, automatic rollback, crash isolation, recovery environment;
   failure is expected and handled, not denied.
3. **Fast on modest hardware** — explicit performance budgets, lazy services, native code for
   core utilities.
4. **Coherent** — one design system, one settings model, one permission model, one update model.
5. **Open to the world** — Linux apps, Flatpak, web apps, developer containers, and isolated
   Windows compatibility where legal and practical.
6. **International** — Unicode, RTL, locale-aware formatting, input methods, and translation from
   the beginning.

## 7. Long-term success indicators

Measured, not asserted:

- percentage of updates applied without user intervention and without rollback;
- automatic rollback success rate in fault-injection tests;
- idle CPU / RAM of a fresh session on reference hardware against budget;
- number of hardware models at "Tested" or "Certified" tier with public evidence;
- median time from upstream CVE publication to shipped fix;
- accessibility audit pass rate per release;
- document round-trip fidelity scores for NUR Office test corpora.

## 8. Naming

"NUR OS" and the NUR-prefixed component names are working names pending trademark clearance
([RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md)).
