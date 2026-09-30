# NUR OS — Roadmap

Status: **PROPOSED.** Calendar durations are deliberately not given: they depend on team size and
funding, which are not yet defined ([RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md)). Relative
effort is indicated as S / M / L / XL.

## Structure

- Phases are **gated**, not strictly sequential: work for a later phase may start early (e.g.
  design-system work during Phase 2), but a phase is **complete** only when its exit criteria and
  gates pass.
- Adjustments to the originally proposed boundaries, with reasons:
  1. **Security foundations move into Phase 1** (signed UKI, dm-verity, A/B prototype, test-key
     Secure Boot in QEMU). The update model and boot chain shape the image layout; retrofitting
     them later would force a redesign. Phase 5 remains the application sandbox + permission phase.
  2. **Developer Preview 0.1 is the gate at the end of Phase 7** (it needs installer, desktop,
     sandbox, update, and minimal core apps).
  3. **Recovery** is part of Phase 4 (with the installer), because an installable system without
     a recovery path must not be handed to testers.

## Phase overview

| Phase | Name | Effort | Key output |
| --- | --- | --- | --- |
| 0 | Architecture & foundation | S | This repository: docs, ADRs, validation, CI |
| 1 | Bootable NUR base | L | Reproducible signed image booting in QEMU with Secure Boot, verity, A/B prototype |
| 2 | Core system & services | L | Hardened service baseline, NUR service skeleton pattern, networking/audio/power, `nur-updated` v0 |
| 3 | NUR Desktop foundation | XL | Compositor, shell, greeter, lock screen, design system v1, toolkit decision |
| 4 | Installer, recovery, hardware enablement | L | Installer, FDE/TPM enrolment, recovery environment, hardware lab, HCDB v0 |
| 5 | Security & application sandbox | L | `nur-permissiond`, NUR portal backend, trusted prompts, MAC profiles, audit log |
| 6 | Store / package / update ecosystem | L | TUF-based updater + Store metadata, NUR runtime/SDK v0, Software app v0, staged rollout |
| 7 | Core NUR applications → **DP 0.1** | L | Settings, Files, Terminal, Software, Update UI; Developer Preview 0.1 |
| 8 | NUR AI platform | L | Broker, intents, provider API, local + cloud providers, AI app |
| 9 | NUR Office | XL | Writer/Sheets/Slides MVP on chosen engine; PDF tools |
| 10 | Developer ecosystem | M | Public SDK, docs, developer identity, Store submission pipeline |
| 11 | Beta & hardware expansion | L | Public Beta, expanded lab, external security audit, performance hardening |
| 12 | NUR OS 1.0 | M | Stable release with support commitments |

---

## Phase 0 — Architecture & Foundation

- **Objective:** authoritative architecture, decision records, repository conventions, validation.
- **Deliverables:** documents listed in [README.md](../README.md); ADRs 0000–0015; CI running
  repository validation; decision register.
- **Dependencies:** none.
- **Exit criteria:** leadership reviews and approves or amends the decisions in
  [RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md) §1 (at minimum: base userspace, project
  license, Secure Boot signing path, team/budget for Phase 1).
- **Tests:** `tools/check_repo.py` and its unit tests pass in CI.
- **Security gates:** no secrets in repository; CI permissions read-only.
- **Performance gates:** n/a.
- **Major risks:** architecture approved without the staffing to execute it.

## Phase 1 — Bootable NUR Base

- **Objective:** a minimal, reproducible, signed NUR image that boots to a console/login in
  QEMU with the target boot chain.
- **Deliverables:** `image/` mkosi config (x86-64; aarch64 build-only); NUR kernel config on the
  selected LTS; signed UKI (CI test keys); `/usr` erofs + dm-verity; systemd-repart layout with
  A/B slots; systemd-boot with boot counting; LUKS2 + swtpm-backed TPM unlock in VM; QEMU smoke
  test harness; first Rust workspace skeleton with CI (fmt/clippy/test/deny, x86_64 + aarch64);
  update spike result (A/B-sysupdate vs bootc) recorded in ADR-0008; base-userspace spike result
  recorded in ADR-0003.
- **Dependencies:** Phase 0 approvals (base userspace, Secure Boot signing path).
- **Exit criteria:** CI builds the image from a clean checkout; QEMU boots with Secure Boot on
  and verity active; a corrupted `/usr` block causes fail-closed behaviour; an update from build
  N to N+1 via A/B slot works in VM; a bad image rolls back automatically; two independent builds
  produce the same image or differences are enumerated.
- **Tests:** layers 1, 3, 4 (subset), 9 (prototype) of [TESTING_STRATEGY.md](TESTING_STRATEGY.md).
- **Security gates:** no unsigned boot path in the production profile; module signing enforced;
  lockdown active; test keys cannot be used by release workflow.
- **Performance gates:** baseline boot time recorded (no target enforced yet).
- **Major risks:** tooling immaturity (sysupdate/repart edge cases); image size; Secure Boot
  signing decision delays.

## Phase 2 — Core System & Services

- **Objective:** a hardened, lazily-activated service baseline and the NUR service pattern.
- **Deliverables:** hardened unit templates + `systemd-analyze security` CI threshold; NetworkManager,
  PipeWire/WirePlumber, BlueZ, UPower, power-profiles-daemon, fwupd, resolved, nftables default
  ruleset, polkit baseline; `nur-updated` v0 (local source, no TUF yet); `org.nur.*` interface
  conventions and first IDL definitions; structured logging conventions; systemd-oomd and slice
  configuration; MAC decision ([ADR-0015](adr/0015-mandatory-access-control.md)) + first profiles.
- **Dependencies:** Phase 1.
- **Exit criteria:** all NUR units within exposure threshold; services start on demand; idle
  system (no session) meets provisional CPU/RAM measurements; integration tests for each NUR
  service's D-Bus API.
- **Tests:** layers 1, 2, 3, 6 (unit hardening), 7 (VM baseline).
- **Security gates:** no NUR service runs as root without an ADR-recorded justification; fuzzing
  set up for every NUR parser of untrusted input.
- **Performance gates:** idle wakeups and RAM measured and tracked; regressions > 10 % fail CI.
- **Major risks:** MAC policy effort; upstream service behaviour under strict hardening.

## Phase 3 — NUR Desktop Foundation

- **Objective:** a usable Wayland session with NUR's own compositor and shell.
- **Deliverables:** UI toolkit decision ([ADR-0006](adr/0006-ui-toolkit.md)) after prototype
  comparison; `nur-compositor` (Smithay, pending approval) with multi-monitor, fractional scaling,
  XWayland; panel/dock, launcher, notifications, quick settings, lock screen, greeter; design
  system v1 (tokens, core components); i18n pipeline with pseudo-locales; a11y baseline (AT-SPI,
  keyboard navigation, screen reader works for shell).
- **Dependencies:** Phase 2 (logind, PipeWire, NetworkManager APIs).
- **Exit criteria:** session survives crash of any shell component; compositor passes protocol
  conformance and fuzz tests; screen reader can operate launcher, panel, notifications, lock
  screen; RTL pseudo-locale renders correctly.
- **Tests:** layers 1–4, 7, 11, 12.
- **Security gates:** no global input/screen access for ordinary clients; lock screen cannot be
  bypassed by crashing it (session-lock protocol semantics tested); trusted-prompt layer design
  reviewed.
- **Performance gates:** login-to-desktop and idle session budgets measured on VM; frame pacing
  measured.
- **Major risks:** compositor scope (largest engineering risk in the project); toolkit
  accessibility maturity; GPU driver diversity.

## Phase 4 — Installer, Recovery & Hardware Enablement

- **Objective:** install on real hardware safely and recover from failure.
- **Deliverables:** live image + installer (disk selection, A/B layout, LUKS2 with TPM2+PIN /
  passphrase, recovery key, user creation, locale/keyboard); recovery environment (reinstall slot,
  factory reset, unlock with recovery key); hardware lab with reference devices; HCDB schema v0 +
  `nur-hwreport` prototype; NVIDIA strategy implemented per decision; fwupd integration.
- **Dependencies:** Phases 1–3; hardware budget approval.
- **Exit criteria:** automated install/reinstall/recover tests in QEMU; every reference device
  reaches "Tested" or has documented known issues; suspend/resume ×20 passes on Tested laptops.
- **Tests:** layers 3, 4, 5, 10.
- **Security gates:** installer never writes unencrypted user data; recovery key flow tested;
  installer does not disable Secure Boot.
- **Performance gates:** boot budgets measured on reference hardware.
- **Major risks:** firmware diversity; TPM quirks; NVIDIA + Secure Boot.

## Phase 5 — Security & Application Sandbox

- **Objective:** enforceable, understandable permission model for apps.
- **Deliverables:** `nur-permissiond`; `xdg-desktop-portal-nur` (file chooser, screencast,
  camera, location, notifications, background, secret); trusted prompter + indicators; Security
  Center (Settings panel); audit log (`nur-auditd`); keyring (oo7) integration; firewall UI basics.
- **Dependencies:** Phases 2–3.
- **Exit criteria:** sandbox escape test suite passes; permission revocation takes effect
  immediately (stream cut); prompts cannot be spoofed by test malicious app; audit entries for all
  sensitive grants.
- **Tests:** layers 2, 3, 6, 11.
- **Security gates:** threat-model review of every portal; fuzzing of portal request parsing;
  external review of permissiond design.
- **Performance gates:** portal round-trip latency budget (target < 50 ms without prompt).
- **Major risks:** app compatibility with strict defaults; upstream portal gaps (USB,
  contacts); XWayland isolation.

## Phase 6 — Store / Package / Update Ecosystem

- **Objective:** trustworthy delivery of OS updates and apps.
- **Deliverables:** TUF repositories (OS + Store) with offline root ceremony (test hierarchy first);
  `nur-updated` v1 (TUF, staged rollout, channels, health checks, security floor); NUR runtime/SDK
  v0 (Flatpak); Store manifest v0 + validation tool; Software app v0 (install/remove/update Flatpaks
  from NUR repo); SBOM + provenance for images and packages.
- **Dependencies:** Phases 1, 2, 5.
- **Exit criteria:** freeze, rollback, and mix-and-match attacks detected in tests; staged rollout
  halt works; interrupted update tests pass (power cut at each step); permission-increase
  re-consent works.
- **Tests:** layers 3, 6, 8, 9.
- **Security gates:** key ceremony executed for test hierarchy; production signing path requires
  two-person approval; release workflow cannot access keys from PRs.
- **Performance gates:** update download/apply time and disk usage measured.
- **Major risks:** operational complexity of key management; delta updates.

## Phase 7 — Core NUR Applications → Developer Preview 0.1

- **Objective:** the minimum application set for the DP 0.1 experience.
- **Deliverables:** Settings (all DP panels), Files, Terminal, Software, Update UI; browser
  integration per decision; screenshot tool; DP 0.1 release per
  [RELEASE_STRATEGY.md §7](RELEASE_STRATEGY.md#7-nur-os-developer-preview-01--definition);
  Office licensing review + fidelity corpus started (no distribution).
- **Dependencies:** Phases 3–6.
- **Exit criteria:** all 14 DP 0.1 acceptance criteria pass; release notes list real status.
- **Tests:** all applicable layers; manual a11y + localisation walkthroughs.
- **Security gates:** security gate of [TESTING_STRATEGY.md §5](TESTING_STRATEGY.md#5-gates).
- **Performance gates:** provisional budgets met on reference hardware or waivers documented.
- **Major risks:** scope creep in apps; desktop polish expectations.

## Phase 8 — NUR AI Platform

- **Objective:** a useful assistant that is safe by construction.
- **Deliverables:** `nur-ai-broker`, `nur-ai-agent`, intent registry, risk tiers, taint tracking,
  confirmations via trusted prompter, Provider API with ≥ 1 local and ≥ 1 cloud adapter, AI app,
  AI activity history ([AI_SECURITY.md](AI_SECURITY.md)).
- **Dependencies:** Phases 5–7.
- **Exit criteria:** prompt-injection corpus: zero unconfirmed R3+ actions; provider swap without
  code change in callers; works with no provider configured (graceful).
- **Tests:** policy unit tests, fuzzing, red-team suite, integration with Files/Settings intents.
- **Security gates:** external AI red-team; privacy review of provider data flows; legal review of
  data residency.
- **Performance gates:** local model resource limits (RAM, GPU memory) enforced; no idle cost
  when unused.
- **Major risks:** prompt injection is an unsolved problem industry-wide — mitigated by
  architecture, not solved; local model quality on modest hardware; provider costs.

## Phase 9 — NUR Office

- **Objective:** Writer, Sheets, Slides MVP with high-fidelity OOXML/ODF and PDF tools.
- **Deliverables:** engine decision ([ADR-0014](adr/0014-office-suite-foundation.md)) after legal
  sign-off; `org.nur.Documents1`; sandboxed per-document workers; three apps MVP; PDF viewer/tools;
  AI intents for documents.
- **Dependencies:** Phases 5, 7, 8 (for AI integration only); legal review.
- **Exit criteria:** fidelity corpus scores above agreed thresholds; malicious-document suite
  contained by sandbox.
- **Tests:** fidelity regression, fuzzing, sandbox tests, a11y, localisation.
- **Security gates:** macros off by default; per-document sandbox verified.
- **Performance gates:** open time and memory for corpus documents.
- **Major risks:** fidelity expectations vs. Microsoft Office; engine size; license obligations.

## Phase 10 — Developer Ecosystem

- **Objective:** third parties can build, sign, and publish NUR apps.
- **Deliverables:** public SDK + docs site; API stability policy in force; developer identity and
  signing; Store submission pipeline with automated checks and review; sample apps.
- **Dependencies:** Phase 6.
- **Exit criteria:** an external developer publishes an app end-to-end in a pilot.
- **Tests:** SDK API compatibility tests; pipeline security tests.
- **Security gates:** malware review pipeline operational; takedown procedure tested.
- **Performance gates:** SDK app template meets startup budget.
- **Major risks:** ecosystem adoption; review staffing.

## Phase 11 — Beta & Hardware Expansion

- **Objective:** public Beta on a broad, measured hardware set.
- **Deliverables:** Beta channel; expanded lab; HCDB public view; external penetration test and
  remediation; performance hardening; translation completeness for launch locales; accessibility
  audit.
- **Dependencies:** Phases 1–10 (AI/Office may ship as preview components).
- **Exit criteria:** crash-free session rate, update success rate, and rollback rate meet targets
  set at Phase 11 start; no open Critical/High security findings.
- **Tests:** all layers, at scale.
- **Security gates:** external audit findings resolved or accepted by leadership in writing.
- **Performance gates:** budgets met on all Certified devices.
- **Major risks:** long tail of hardware bugs; support load.

## Phase 12 — NUR OS 1.0

- **Objective:** Stable release with support commitments.
- **Deliverables:** Stable channel; support policy in force; security advisory process live;
  certified hardware list; documentation for users and administrators.
- **Dependencies:** Phase 11 exit.
- **Exit criteria:** release gate passes; support and incident-response staffing in place.
- **Tests / security / performance gates:** full RC gate.
- **Major risks:** premature 1.0 under commercial pressure — the gate must not be waived for
  schedule reasons.

## After 1.0 (not planned in detail)

ARM64 releases, enterprise management, NUR Cloud sync, collaborative Office, HDR/VRR/colour
management, Windows compatibility polish, LTS/Enterprise channel.
