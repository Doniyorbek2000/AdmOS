# ADR-0012: NUR AI privilege boundary

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

NUR AI will perform multi-step tasks over user data (search, analyse, create documents). Large
language models can be wrong and can be manipulated by content they read (prompt injection). If
the AI has ambient authority, any manipulated output becomes a system compromise.

## Decision

This ADR fixes the **principle**; component design is PROPOSED in
[AI_SECURITY.md](../AI_SECURITY.md).

1. NUR AI never runs with root/admin privileges and never has ambient user authority.
2. All AI effects occur through **typed intents** exposed by NUR APIs and executed by a separate
   policy broker; there is no generic shell, arbitrary-path, or arbitrary-IPC tool.
3. Actions are classified into risk tiers; destructive, external-egress, installation, and
   privileged actions require explicit confirmation on the trusted path; security settings and
   credentials are never exposed to AI.
4. Content from files, web, and tool output is untrusted data; actions derived from it are
   taint-tracked.
5. All AI actions are audit-logged (metadata only).
6. AI providers (cloud and local) are pluggable behind a NUR Provider API; no provider is
   hard-coded; the OS works without any provider.

## Consequences

- Prompt injection can at most cause actions the user confirms or low-risk reversible actions.
- Some workflows need more confirmations than a fully autonomous agent would.
- Every app that wants AI integration must define intents with schemas and tiers.

## Alternatives considered

- AI as a privileged system agent with command execution: rejected (single point of total
  compromise).
- AI with the user's full session authority: rejected (prompt injection = user compromise).

## Revisit triggers

None for the principle. Tier details evolve through AI_SECURITY.md.
