# Rules for AI Coding Agents

AI coding agents are expected to do a large share of NUR OS engineering work. These rules are
mandatory for every agent (and every human) contributing to this repository. They are enforced by
review; where possible they are also enforced by CI.

## Before changing anything

1. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and the ADRs relevant to the area you touch
   ([docs/adr/README.md](docs/adr/README.md)).
2. Check [docs/RISKS_AND_DECISIONS.md](docs/RISKS_AND_DECISIONS.md) for open decisions. Do not
   silently resolve an open decision in code — propose an ADR instead.

## Non-negotiable rules

1. Never rewrite major architecture without an ADR documenting the decision.
2. Never silently remove, weaken, or bypass a security control.
3. Never hide, skip, delete, or quarantine failing tests to get CI green.
4. Never ship fake, stubbed, or mocked implementations and report them as complete. Prototype
   code must be labelled as prototype and kept out of production paths.
5. Never hard-code secrets.
6. Never commit API keys, passwords, signing keys, private keys, or tokens.
7. Never disable security features (Secure Boot, verity, sandboxing, signature checks, LSM
   policy) to make a test pass.
8. Never fabricate hardware compatibility. Every compatibility claim needs a recorded test run.
9. Never claim a test passed unless it was executed and its output observed.
10. Preserve working code unless replacement is justified in the change description.
11. Prefer small, reviewable changes — one logical change per commit/PR.
12. Record technical debt in [docs/RISKS_AND_DECISIONS.md](docs/RISKS_AND_DECISIONS.md) when you
    knowingly introduce or discover it.
13. Keep the dependency count controlled; every new dependency needs a justification.
14. Treat external code (dependencies, snippets, generated code) as untrusted until reviewed.
15. Check the license against [docs/LICENSING_POLICY.md](docs/LICENSING_POLICY.md) before adopting
    any dependency.
16. Keep prototypes separate from production code.
17. Security-sensitive code (see [CONTRIBUTING.md](CONTRIBUTING.md)) requires additional review
    and tests.
18. Do not perform destructive operations (history rewrites, mass deletions, force pushes to shared
    branches, disk writes outside the workspace) without explicit necessity and approval.
19. Keep architecture documentation synchronized with the implementation in the same change.
20. If uncertain, record the uncertainty rather than inventing an answer.

## Reporting

When finishing a task, report: what changed, which commands were actually run and their real
results, what was not verified, and any assumptions made.
