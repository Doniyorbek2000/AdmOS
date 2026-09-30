# NUR OS — Testing Strategy and CI

Status: **Phase 0**: only repository validation exists (`tools/check_repo.py` + unit tests). All
other layers below are planned and must be introduced no later than the phase indicated.

## 1. Principles

1. **No release relies on "it works on my machine."** Every release artifact is produced and
   tested by CI from a tagged commit.
2. **Tests are never skipped, disabled, or quarantined to get green** ([AGENTS.md](../AGENTS.md)).
   A known failure is tracked as a bug with an owner and deadline; the gate stays red or an
   explicit, reviewed, time-limited waiver is recorded in the release notes.
3. **Test what ships**: system tests run against the signed image, with Secure Boot and TPM
   emulation enabled — never against a "debug" image with protections off.
4. Reported results are the real results. Flakiness is a bug, not an excuse.

## 2. Test layers

| # | Layer | Scope | Tooling (planned) | Runs | Introduced |
| --- | --- | --- | --- | --- | --- |
| 1 | Unit | Functions/modules | `cargo test`, `unittest` for tooling, Miri for `unsafe` | Every PR | Phase 0 (tooling), Phase 1 (Rust) |
| 2 | Integration | Service + its D-Bus API, portal flows | Test harness spawning services on private buses, `systemd-nspawn` | Every PR | Phase 2 |
| 3 | System | Whole image: boot, services, session | QEMU/KVM + OVMF (Secure Boot) + swtpm | Every merge to main; PRs touching image | Phase 1 |
| 4 | VM matrix | Firmware variants, CPU models, disk types, RAM sizes, no-TPM | QEMU profiles | Nightly | Phase 1 |
| 5 | Hardware | Real devices from lab matrix | Lab runner agents, PiKVM-style remote control (research) | Per release candidate; nightly subset later | Phase 4 |
| 6 | Security | Sandbox escapes, permission enforcement, fuzzing, unit hardening, dependency audit, secrets scan | cargo-fuzz, `systemd-analyze security`, custom sandbox probes, cargo-deny | Every PR (fast), nightly (fuzzing) | Phase 1 → 5 |
| 7 | Performance | Boot time, login time, idle CPU/RAM, frame timing, app launch | `systemd-analyze`, PSI/cgroup stats, compositor frame traces | Nightly on fixed VM + reference hardware | Phase 2 (VM), Phase 4 (hardware) |
| 8 | Upgrade | N-1 → N, N-2 → N, channel switches | QEMU with real published images | Every release candidate | Phase 6 (Phase 1 prototype) |
| 9 | Rollback | Bad image → auto fallback; manual rollback; state migration reverse | Fault-injected images (fail health check, kernel panic) | Every release candidate | Phase 1 prototype, Phase 6 gate |
| 10 | Recovery | Recovery boot, re-install slot, factory reset, recovery key unlock | QEMU | Every release candidate | Phase 4 |
| 11 | Accessibility | AT-SPI tree completeness, keyboard traversal, contrast, screen-reader walkthrough | Automated a11y tree checks + manual | Per PR for UI (automated), per RC (manual) | Phase 3 |
| 12 | Localisation | Pseudo-locales (long strings, RTL), missing translations, locale formatting | Pseudo-locale builds, string extraction checks | Per PR for UI | Phase 3 |

## 3. QEMU smoke test (Phase 1 target)

```text
build image (mkosi) → sign with CI test keys (never production keys)
  → qemu-system-x86_64 -machine q35 -enable-kvm
      -drive OVMF_CODE.secboot + OVMF_VARS with NUR test keys enrolled
      -tpmdev swtpm
      -serial / QMP control socket
  → assert: Secure Boot state = enabled (from guest)
  → assert: /usr mounted via dm-verity; root hash matches build output
  → assert: systemctl is-system-running == running (not degraded)
  → assert: no journal messages with priority ≤ err from NUR components
  → assert: boot marked good (boot counting)
  → collect: journal, systemd-analyze blame/critical-chain → artifacts
  → poweroff within timeout
```

Failure-injection variants (Phase 1–2): corrupt a verity block → boot must fail closed and fall
back; image failing health check → automatic rollback after N tries.

GUI tests (Phase 3+): openQA-style screen matching or Wayland-protocol-driven test clients;
decision in Phase 3.

## 4. CI pipeline (evolving)

| Stage | Phase 0 (now) | Later |
| --- | --- | --- |
| Formatting | Whitespace/final-newline check | rustfmt, other formatters |
| Linting | — | clippy `-D warnings`, shellcheck, systemd unit lint, actionlint |
| Docs integrity | Required documents, ADR metadata, relative links + anchors | Same + spelling (codespell) |
| Secrets | Pattern scan in `check_repo.py` | + gitleaks (or equivalent) with history scan |
| Compilation | — | `cargo build` for x86_64 **and** aarch64 |
| Unit tests | Tooling tests | All crates |
| Dependency audit | — | cargo-deny (advisories, licenses, bans, sources) |
| SBOM | — | Per image and package |
| License checks | — | REUSE lint for NUR sources, ScanCode for third-party |
| Reproducibility | — | Rebuild image twice in different environments, diff (diffoscope) |
| Image generation | — | mkosi image per channel |
| QEMU boot tests | — | §3 |
| Installer tests | — | Automated install into blank VM disk, reboot, unlock |
| Update/rollback tests | — | §2 layers 8–9 |

CI security: actions pinned to commit SHAs (currently tag-pinned — tracked debt in
[RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md)), `permissions: contents: read` by default, no
production secrets available to PR builds, signing performed only in a separate protected
release workflow.

## 5. Gates

| Gate | Blocks |
| --- | --- |
| PR gate | Merge to `main` |
| Nightly gate | Publication to Developer channel |
| Release-candidate gate (all 12 layers applicable to the phase) | Beta/Stable publication |
| Security gate (no open Critical/High without accepted mitigation) | Any release |
| Performance gate (no budget regression > threshold without approved waiver) | Beta/Stable |

## 6. Hardware lab (Phase 4+)

- Devices from [HARDWARE_COMPATIBILITY.md](HARDWARE_COMPATIBILITY.md) §4 with remote power, serial
  or network console, and video capture.
- Results feed the HCDB automatically with build IDs and logs.
- Suspend/resume stress, battery drain, and thermal tests scheduled weekly on laptops.
