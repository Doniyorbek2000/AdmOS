# NUR OS — Repository Strategy

Status: **DECIDED for Phases 0–2** ([ADR-0011](adr/0011-monorepo.md)); revisited at the end of
Phase 2 or when a trigger in the ADR fires.

## 1. Decision summary

**One monorepo for all NUR-authored code, image definitions, and documentation.** Upstream
projects are consumed as pinned artifacts (packages, tarballs by digest) — not vendored forks —
except where a fork is unavoidable.

Why:

- Cross-cutting changes (a D-Bus API + its service + its client + its docs + its ADR) land
  atomically and are reviewed together.
- One CI definition, one dependency policy, one lockfile for Rust (Cargo workspace).
- Architecture documentation lives next to the code it constrains, which makes drift visible.
- The team is small; multi-repo coordination overhead is not justified yet.

Costs accepted: CI must be path-aware to stay fast; access control is coarser (CODEOWNERS
compensates); the repository will grow (large binary assets are banned — see §4).

## 2. Current layout (Phase 0)

```text
.
├── AGENTS.md / CLAUDE.md     rules for AI agents
├── CONTRIBUTING.md           workflow, review, commit conventions
├── README.md
├── SECURITY.md               vulnerability reporting
├── docs/                     authoritative architecture documents
│   └── adr/                  Architecture Decision Records
├── tools/                    repository tooling (validation)
│   └── tests/
└── .github/workflows/        CI
```

## 3. Planned layout (created only when real content lands)

A directory is created in the change that adds its first real, reviewed content — never as an
empty placeholder.

| Directory | Content | Expected phase |
| --- | --- | --- |
| `image/` | mkosi configuration, repart definitions, kernel config fragments, package lists per arch | 1 |
| `crates/` | Rust workspace members (services, libraries), one crate per directory | 1–2 |
| `crates/nur-updated`, `crates/nur-permissiond`, ... | System services | 2, 5, 6 |
| `desktop/` | Compositor, shell components, greeter | 3 |
| `apps/` | First-party applications (Settings, Files, Terminal, Software) | 3, 7 |
| `sdk/` | Runtime/SDK manifests, API definitions (D-Bus XML / IDL), bindings | 5, 10 |
| `installer/` | Installer and live environment | 4 |
| `recovery/` | Recovery environment | 4 |
| `policy/` | polkit rules, MAC profiles, seccomp profiles, default permission policy | 2, 5 |
| `tests/` | System/VM test suites (unit tests stay beside code) | 1 |
| `hardware/` | HCDB schema, test-case definitions, lab tooling | 4 |
| `ai/`, `office/`, `store/` | Later phases | 6–9 |

Components are grouped by **what they are** (crate, image, desktop, app), not by marketing name,
to avoid shuffling code when products are renamed.

## 4. Rules

1. No large binaries in git (ISOs, images, model weights, fonts > 1 MB, video). Use release
   artifact storage; reference by digest.
2. No vendored third-party source without an ADR (reason, update process, license).
3. Upstream patches live in `image/patches/<project>/` with a header linking the upstream
   submission; every patch has an owner and a removal condition.
4. Generated files are not committed unless required for bootstrapping, and then are verified
   by CI to match their generator.
5. Branch protection on `main` (to be configured by repository admins): required reviews, required
   CI, linear history, no force-push, signed commits (decision pending).

## 5. When to split repositories

Split a component out only when at least one holds (record with an ADR):

- it needs a different access-control boundary (e.g. signing infrastructure, private
  commercial services, security embargo work);
- it has an independent release cadence **and** an independent consumer base (e.g. public SDK);
- repository size or CI time can no longer be managed with path filtering and caching.

Anticipated separate repositories later: signing/release infrastructure configuration (private),
Store/cloud backend services (private), HCDB data, and possibly the public SDK documentation site.
