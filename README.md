# NUR OS

NUR OS is a long-term desktop operating system platform developed by **ADM Tech Holding MChJ**.
It uses a mature Linux LTS kernel and the Linux hardware ecosystem as its low-level foundation
and builds a coherent, security-first platform on top of it: NUR Core, NUR Security, system
services, stable platform APIs, NUR Desktop, and first-party applications.

> **Status: Phase 0 — Architecture & Foundation.**
> There is no bootable image, no desktop, and no application code yet. This repository currently
> contains architecture documents, Architecture Decision Records (ADRs), and repository
> validation tooling. Nothing in this repository should be read as a claim that a feature exists
> unless the document explicitly says it is implemented and tested.

## Where to start

| Document | Purpose |
| --- | --- |
| [docs/VISION.md](docs/VISION.md) | What NUR OS is, who it is for, and what it is not |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Layered system architecture and component boundaries |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phases 0–12, exit criteria, gates, and Developer Preview 0.1 |
| [docs/TECH_STACK.md](docs/TECH_STACK.md) | Selected, proposed, and under-research technologies |
| [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) | Security architecture and trust boundaries |
| [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) | Assets, adversaries, attack surfaces, mitigations |
| [docs/UPDATE_ARCHITECTURE.md](docs/UPDATE_ARCHITECTURE.md) | Atomic, signed, rollback-capable updates |
| [docs/APP_PLATFORM.md](docs/APP_PLATFORM.md) | App model, sandbox, permissions, compatibility, NUR Store |
| [docs/DESKTOP_ARCHITECTURE.md](docs/DESKTOP_ARCHITECTURE.md) | Wayland compositor, shell, design system, i18n, accessibility |
| [docs/HARDWARE_COMPATIBILITY.md](docs/HARDWARE_COMPATIBILITY.md) | Hardware strategy and compatibility database design |
| [docs/AI_SECURITY.md](docs/AI_SECURITY.md) | NUR AI privilege boundary and provider architecture |
| [docs/OFFICE_STRATEGY.md](docs/OFFICE_STRATEGY.md) | NUR Office foundation research and licensing |
| [docs/TESTING_STRATEGY.md](docs/TESTING_STRATEGY.md) | Test layers, QEMU automation, hardware lab |
| [docs/RELEASE_STRATEGY.md](docs/RELEASE_STRATEGY.md) | Versioning, channels, signing, release gates |
| [docs/LICENSING_POLICY.md](docs/LICENSING_POLICY.md) | Dependency license policy and obligations |
| [docs/REPOSITORY_STRATEGY.md](docs/REPOSITORY_STRATEGY.md) | Monorepo layout and growth rules |
| [docs/RISKS_AND_DECISIONS.md](docs/RISKS_AND_DECISIONS.md) | Open decisions requiring approval, risks, assumptions, tech debt |
| [docs/adr/README.md](docs/adr/README.md) | Architecture Decision Records index |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute, review rules, commit conventions |
| [AGENTS.md](AGENTS.md) | Mandatory rules for AI coding agents working in this repository |
| [SECURITY.md](SECURITY.md) | How to report a vulnerability |

## Validation

The repository ships a dependency-free validation tool (Python 3.11+ standard library only):

```bash
python3 tools/check_repo.py            # structure, ADR metadata, links, secret patterns, whitespace
python3 -m unittest discover -s tools/tests -v
```

Both run in CI on every push and pull request (`.github/workflows/ci.yml`).

## License

**Not yet decided.** No license has been granted for the contents of this repository. Until
ADM Tech Holding MChJ approves a project license (see
[docs/LICENSING_POLICY.md](docs/LICENSING_POLICY.md) and
[docs/RISKS_AND_DECISIONS.md](docs/RISKS_AND_DECISIONS.md)), all rights are reserved by the
copyright holder.

"NUR OS" and related names are working names. Trademark clearance has not been performed.
