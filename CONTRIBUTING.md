# Contributing to NUR OS

NUR OS is in Phase 0. Contributions today are architecture reviews, ADRs, and tooling. This
document defines how work enters the repository now and as code arrives.

All contributors — human and AI — must also follow [AGENTS.md](AGENTS.md).

## 1. Workflow

1. **Discuss first** for anything non-trivial: open an issue describing the problem, not just the
   solution.
2. **Architecture changes need an ADR.** If a change alters a component boundary, a public API,
   the security model, the update model, a major dependency, or contradicts an existing ADR,
   write or update an ADR in [docs/adr/](docs/adr/README.md) in the same pull request.
3. **Branch** from `main`: `<type>/<short-topic>` (e.g. `feat/permission-store`,
   `docs/threat-model-usb`).
4. **Keep pull requests small**: one logical change; aim for < 400 changed lines of non-generated
   code.
5. **Run validation locally** before pushing (see below). CI must be green before review.
6. **Merge** with a linear history (squash or rebase merge) once approvals are in place.

## 2. Commit messages

[Conventional Commits](https://www.conventionalcommits.org/) style:

```text
<type>(<scope>): <imperative summary, ≤ 72 chars>

<body: what and why, wrapped at 72 columns>

Refs: #123
```

Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `security`,
`revert`. Scopes follow top-level components (`core`, `security`, `update`, `desktop`, `ai`, ...).

Sign-off policy (DCO vs. CLA) is an open decision — see
[docs/RISKS_AND_DECISIONS.md](docs/RISKS_AND_DECISIONS.md).

## 3. Review requirements

| Change class | Required approvals | Extra requirements |
| --- | --- | --- |
| Documentation (non-architectural) | 1 | — |
| Regular code | 1 | Tests for new behaviour |
| ADR (new or status change) | 2, including an architecture owner | Discussion period ≥ 2 business days |
| **Security-sensitive code** | 2, including a security owner | Threat-model delta, negative tests, fuzzing where input parsing is involved |
| New third-party dependency | 1 + license check | Justification, license, maintenance status, alternatives |
| Release/signing/CI infrastructure | 2, including a release owner | No secrets in logs; pinned actions |

**Security-sensitive** means any code that: runs with elevated privileges or capabilities; parses
untrusted input in a privileged context; implements authentication, authorization, permissions,
sandboxing, cryptography, signature verification, key handling, update installation, boot chain,
disk encryption, the AI action broker, or audit logging.

CODEOWNERS will encode these owners once the team roster is defined.

## 4. Validation

```bash
python3 tools/check_repo.py
python3 -m unittest discover -s tools/tests -v
```

As language toolchains are introduced, their standard checks become mandatory:

- Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  `cargo deny check` (licenses, advisories, bans, sources).
- Shell: `shellcheck`; scripts start with `set -euo pipefail`.
- Python tooling: standard library preferred; type hints required.

## 5. Code standards

- New privileged or security-sensitive components are written in **Rust** unless an ADR justifies
  otherwise ([ADR-0007](docs/adr/0007-memory-safe-languages.md)).
- `unsafe` Rust requires a `// SAFETY:` comment explaining the invariant, and security review.
- No user-facing strings hard-coded in logic: route them through the localization system.
- Errors are handled, never swallowed. Logs are structured (journald fields), never contain
  secrets or personal content.
- Every long-running service ships a hardened systemd unit (see
  [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md)).

## 6. Documentation

Architecture documents are authoritative. If code and docs disagree, the change that caused the
drift is incomplete. Update docs in the same pull request.

## 7. Reporting security issues

See [SECURITY.md](SECURITY.md). Never disclose vulnerabilities in public issues.
