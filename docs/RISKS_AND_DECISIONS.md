# NUR OS — Decisions Requiring Approval, Risks, Assumptions, Technical Debt

This is the live register. Update it in the same change that resolves or introduces an item.

## 1. Decisions requiring human approval

These must **not** be decided automatically by engineers or AI agents.

| ID | Decision | Recommendation | Why it needs approval | Blocks |
| --- | --- | --- | --- | --- |
| D1 | Base userspace source | Debian stable packages composed with mkosi ([ADR-0003](adr/0003-base-userspace-and-image-build.md)) | Long-term dependency; affects release cadence, support windows, trademark wording | Phase 1 |
| D2 | Secure Boot path: Microsoft-signed shim vs NUR-only keys | Pursue shim-review for broad out-of-box compatibility; support user/enterprise key enrolment too | Cost, process time with Microsoft/shim-review, relationship with third party; impacts which PCs boot out of the box | Phase 1 (design), Phase 4 (installer) |
| D3 | License for NUR-authored code | Apache-2.0 for platform/SDK/tooling; separate decision for server services ([LICENSING_POLICY.md](LICENSING_POLICY.md)) | Business model, community strategy | Any external contribution; public release |
| D4 | Contributor terms: DCO vs CLA | CLA if relicensing flexibility is needed; otherwise DCO | Legal/business | External contributions |
| D5 | Owned compositor (Smithay) vs adopting KWin/cosmic-comp | Owned Smithay-based compositor ([ADR-0005](adr/0005-compositor-foundation.md)) | Largest single engineering investment; staffing | Phase 3 |
| D6 | UI toolkit | Decide after Phase 3 prototype comparison ([ADR-0006](adr/0006-ui-toolkit.md)) | Long-term ecosystem and hiring impact | Phase 3 |
| D7 | NVIDIA support approach | Open kernel modules built/signed by NUR + proprietary userspace, pending legal review; Nouveau/NVK fallback | Legal redistribution terms; Secure Boot signing of third-party code | Phase 4 |
| D8 | Default browser | Unmodified Firefox via Flatpak (trademark-compliant) or Chromium-based | Trademark policies, search-deal/business implications, maintenance | Phase 7 |
| D9 | Office engine | LibreOffice core via LibreOfficeKit (research) ([ADR-0014](adr/0014-office-suite-foundation.md)) | Licensing obligations, brand, possible commercial license costs | Phase 9 |
| D10 | Reference hardware models and lab budget | See [HARDWARE_COMPATIBILITY.md](HARDWARE_COMPATIBILITY.md) §4 | Budget | Phase 4 |
| D11 | Telemetry policy and data residency (servers location) | Opt-in only; documented schema; data stored per local legal requirements | Legal (personal-data law), trust | Phase 6 (updates metrics), Phase 8 (AI) |
| D12 | Cloud AI provider(s) for first integration | Choose ≥ 1 provider meeting data-handling requirements; keep local option | Contracts, cost, data protection | Phase 8 |
| D13 | Trademark: "NUR", "NUR OS" and component names | Commission trademark search before public announcement | Legal risk; "Nur" is a common word/name | Public announcement |
| D14 | Team size, roles, and funding for Phases 1–4 | Minimum core: OS/image, security, desktop/compositor, services, QA/infra | Roadmap feasibility | Phase 1 |
| D15 | Media codec strategy | Royalty-free codecs by default; licensed codecs as optional installs | Patent licensing per market | Phase 7 |
| D16 | Signed commits requirement on `main` | Require (SSH or GPG signing) | Contributor friction vs provenance | Before external contributors |

## 2. Top risks

| ID | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| R1 | Scope far exceeds staffing (full OS + desktop + AI + Office) | High | Critical | Strict phase gates; DP 0.1 scope fixed; defer AI/Office; hire for compositor early |
| R2 | Owned compositor takes much longer than planned | High | High | Limited Phase 3 scope; fallback option of adopting an existing compositor (decision D5 kept revisitable) |
| R3 | Hardware compatibility gaps (Wi-Fi, webcams, suspend) damage reputation | High | High | Tier system, honest HCL, lab, HWE kernel track |
| R4 | Signing key compromise | Low | Catastrophic | HSM, TUF thresholds, offline root, ceremonies, rotation drills |
| R5 | Supply-chain compromise via upstream packages | Medium | Critical | Pinned snapshots, SBOM diffing, reproducible builds, delayed Stable adoption |
| R6 | License non-compliance (GPL source obligations, trademarks, codecs, Office engine) | Medium | High | Policy, SBOM-driven notices, legal review before distribution |
| R7 | Prompt injection leads to harmful AI actions | Medium | High | Broker architecture, tiers, taint tracking, trusted confirmations, red-team |
| R8 | Office fidelity below user expectations | High | Medium | Fidelity corpus, clear messaging, engine choice driven by measurement |
| R9 | Secure Boot signing path blocked or delayed | Medium | High | Early engagement; user key-enrolment path documented |
| R10 | Base distribution cadence mismatch (old Mesa/kernel for new hardware) | Medium | Medium | NUR builds kernel and Mesa itself; HWE track |
| R11 | Flatpak ecosystem apps request broad permissions, undermining sandbox value | High | Medium | Store policy, install-time risk display, overrides |
| R12 | Regulatory exposure (EU Cyber Resilience Act if sold in EU; Uzbek personal-data law incl. localisation rules) | Medium | High | Legal review before commercial distribution; security-update and vulnerability-handling processes designed to meet CRA-style obligations |
| R13 | AI coding agents introduce subtle security bugs or fake implementations | Medium | High | [AGENTS.md](../AGENTS.md) rules, mandatory human review for security-sensitive code, tests, fuzzing |
| R14 | Single points of knowledge (bus factor) | High | High | Documentation-first culture, ADRs, pairing |

## 3. Assumptions (to be validated)

| ID | Assumption | Validate by |
| --- | --- | --- |
| AS1 | systemd image-based tooling (repart, sysupdate, UKI, cryptenroll, pcrlock) is mature enough for production desktop use on the chosen base | Phase 1 spike |
| AS2 | Debian stable + NUR-built kernel/Mesa provides adequate hardware support for current laptops | Phase 1–4 testing |
| AS3 | Smithay can support the Phase 3 feature set with a small team | Phase 3 prototype milestone |
| AS4 | Flatpak's portal model can express NUR's permission UX without forking Flatpak | Phase 5 design |
| AS5 | GitHub-hosted runners provide KVM for QEMU tests at acceptable speed; otherwise self-hosted runners are needed | Phase 1 |
| AS6 | Target users' hardware mostly has TPM 2.0 and UEFI | Market hardware research (Phase 4) |
| AS7 | LibreOfficeKit can be driven by a native non-LibreOffice UI with acceptable performance | Phase 7 spike |

## 4. Open research questions

Tracked in their ADRs/documents: base userspace (ADR-0003), UI toolkit (ADR-0006), MAC system
(ADR-0015), Office engine (ADR-0014), per-app network control, XWayland isolation, per-user home
encryption, accounts service (homed vs AccountsService), input method framework, message format
(Fluent vs ICU MessageFormat vs gettext), transparency log for releases, delta update format,
local AI runtime, filesystem for state partition (ext4 vs btrfs).

## 5. Technical debt register

| ID | Debt | Introduced | Resolution |
| --- | --- | --- | --- |
| TD1 | CI uses `actions/checkout` pinned by major tag, not commit SHA | Phase 0 | Pin to verified commit SHAs and enable Dependabot/Renovate for action updates |
| TD2 | Secret scanning is a small regex set in `tools/check_repo.py`, not a full scanner with history scan | Phase 0 | Add gitleaks (or equivalent) in Phase 1 |
| TD3 | No CODEOWNERS file (team roster undefined) | Phase 0 | Add when D14 resolved |
| TD4 | Branch protection not configured (requires repository admin) | Phase 0 | Configure per [REPOSITORY_STRATEGY.md](REPOSITORY_STRATEGY.md) §4 |
| TD5 | License facts in docs are from engineering knowledge, not verified against specific upstream versions | Phase 0 | Verify at adoption time; ScanCode in CI |
| TD6 | Performance budgets are provisional and not tied to measured reference hardware | Phase 0 | Ratify in Phase 1/4 |
