# NUR AI — Security Architecture and Privilege Boundary

Status: **Privilege boundary DECIDED** ([ADR-0012](adr/0012-nur-ai-privilege-boundary.md));
component design **PROPOSED**; provider/runtime choices **RESEARCH REQUIRED**. The assistant is
**not** built in Phase 0 and is scheduled for Phase 8 ([ROADMAP.md](ROADMAP.md)).

## 1. Core rule

> NUR AI is an **untrusted, unprivileged requester**. It never holds root, admin, or ambient user
> authority. Every effect it has on the system goes through typed NUR APIs, the permission system,
> and — for high-impact actions — explicit user confirmation on the trusted path.

Model output is treated like input from the network: it can be wrong, manipulated, or malicious.

## 2. Components

```text
┌────────────── user session ──────────────────────────────────────────────────┐
│  NUR AI app (UI)          nur-shell-prompter (trusted confirmations)         │
│        │                              ▲                                      │
│        ▼                              │                                      │
│  nur-ai-agent  ──tool calls──▶  nur-ai-broker  ──▶ NUR Platform APIs          │
│  (sandboxed; no FS, no net        (policy, risk tiers,      (Files, Office,   │
│   except to broker)                taint tracking, audit)    Store, Settings) │
│        │                              │                                      │
│        ▼                              ▼                                      │
│  Provider API ──▶ local runtime (sandboxed)  |  cloud adapter (egress policy)│
└──────────────────────────────────────────────────────────────────────────────┘
```

| Component | Privilege | Responsibility |
| --- | --- | --- |
| **NUR AI app** | Regular sandboxed app | Conversation UI; displays plans and results |
| **`nur-ai-agent`** | Sandboxed, no filesystem, no direct network, no D-Bus except broker | Runs the orchestration loop; turns model output into *proposed* tool calls |
| **`nur-ai-broker`** | Unprivileged session service (not root) | Tool registry; validates calls against schemas; enforces permissions, risk tiers, quotas; requests confirmations; writes audit log; performs calls with the **user's** authority scoped to the grant |
| **Provider adapters** | Sandboxed; network only to the configured endpoint | Translate the Provider API to a specific cloud API or local runtime |
| **Local inference runtime** | Sandboxed; no network; GPU access via device portal/DRM render node | Runs on-device models |
| **`nur-shell-prompter`** | Trusted shell component | Renders confirmations the AI cannot draw, read, or click |

The agent and broker are separate processes so that a compromised agent (e.g. by prompt
injection) still cannot bypass policy.

## 3. Tools = Intents

AI capabilities are exposed as **typed intents** registered by first-party components and later
by apps through the SDK:

```text
intent: files.search         risk: R1  params: {query, scope: [documents|downloads|...], type}
intent: files.read           risk: R1  params: {document_handle}         (handle from search/picker)
intent: sheets.analyze       risk: R1  params: {document_handle, question}
intent: writer.create        risk: R2  params: {title, content_ref}      (new file, reversible)
intent: slides.create        risk: R2  params: {outline_ref}
intent: files.move_to_trash  risk: R3  params: {document_handles[]}      (confirm)
intent: files.share_external risk: R4  params: {handle, destination}     (confirm + egress)
intent: store.install        risk: R4  params: {app_id}                  (confirm + admin policy)
intent: settings.security.*  risk: R5  — not exposed to AI
```

- No generic "run shell command", "read arbitrary path", or "call arbitrary D-Bus method" intent
  exists. A developer-mode terminal intent, if ever added, runs inside a disposable container with
  confirmation for every command.
- Intents operate on **handles** issued by the broker (from search results or user picks), not
  raw paths, so the AI cannot probe the filesystem.

## 4. Risk tiers

| Tier | Examples | Default policy |
| --- | --- | --- |
| R0 | Answer from conversation only; public knowledge | Allowed |
| R1 | Read user data within granted scopes (search/read documents) | Allowed within scopes the user granted to NUR AI (e.g. Documents but not `~/.ssh`); sensitive locations are never grantable |
| R2 | Create new content; reversible modifications (new files, drafts) | Allowed; shown in activity feed; undo available |
| R3 | Destructive or hard-to-reverse (delete, overwrite, send notifications to others) | **Explicit confirmation** per action, summarising exactly what changes |
| R4 | External egress of private data, installing software, spending money, contacting people | **Explicit confirmation** + destination shown; admin policy may forbid |
| R5 | Security settings, credentials, privilege escalation, disabling protections, accessing mic/camera without user-initiated session | **Not available to AI** |

Confirmations cannot be batched into a blanket "allow all" for R3+ (a per-session "allow similar
actions for 10 minutes" option for R3 is a research item with abuse analysis).

## 5. Prompt-injection and taint model

- Content read from files, web pages, emails, or tool results is **untrusted data**. It is passed
  to the model with provenance labels, never merged into system instructions.
- The broker tracks **taint**: if a proposed action's parameters derive from untrusted content
  (e.g. a destination address found in a document), the action requires confirmation even if its
  tier normally would not, and the confirmation shows the provenance.
- Egress guard: any R4 egress includes a data summary (which documents, how many bytes) and the
  destination; destinations first seen in untrusted content are highlighted.
- Rate limits and loop guards: maximum tool calls per task, per minute; abort on repeated
  failures.
- Kill switch: one global control (shell + keyboard shortcut) stops all AI activity immediately.

## 6. Provider architecture

```text
trait Provider {
  capabilities() -> {context_window, tool_calling, vision, streaming, languages}
  complete(request: ChatRequest) -> Stream<ChatEvent>
  embed(texts) -> Vec<Embedding>        (optional)
}
```

- Providers are plugins behind a NUR-owned API; **no provider is hard-coded** and the system
  works (with reduced capability) with none configured.
- **Local provider**: on-device runtime (llama.cpp-class or ONNX Runtime-class; research),
  models downloaded through the Store with signatures and licenses tracked.
- **Cloud providers**: configured by the user or organisation; each shows its data-handling
  policy (retention, training use, region). The user chooses per task class (e.g. "documents:
  local only").
- **Data minimisation**: the broker sends the minimum context; redaction of detected secrets
  (keys, passwords, card numbers) before egress; secrets never enter prompts because the AI has
  no keyring access.
- **Data residency**: organisations can restrict providers by region. Uzbek personal-data law
  implications must be reviewed by counsel ([RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md)).
- Model licenses (weights) are tracked like software licenses ([LICENSING_POLICY.md](LICENSING_POLICY.md)).

## 7. Audit

Every tool call is recorded in the NUR audit log ([SECURITY_MODEL.md §9](SECURITY_MODEL.md#9-audit-logging)):
timestamp, intent, risk tier, parameter *summary* (handles, not content), taint flags, decision
(auto/confirmed/denied), provider used, outcome. Users can view and export the AI activity history.
Conversation content is stored locally, encrypted with the user's keyring, with a retention
setting; it is never written to the audit log.

## 8. Worked example

> "Find my latest spreadsheet, analyse sales, create a report and prepare a presentation."

| Step | Intent | Tier | User sees |
| --- | --- | --- | --- |
| 1 | `files.search {type: spreadsheet, sort: modified}` in granted scope | R1 | Activity: "Searched Documents" |
| 2 | `files.read {handle}` | R1 | Activity: "Read Sales_Q3.xlsx" |
| 3 | `sheets.analyze` (runs in Office service, not in the model) | R1 | — |
| 4 | Model call to provider | — | If cloud: first-time prompt "Send contents of Sales_Q3.xlsx to <Provider>?" per data policy |
| 5 | `writer.create {title: "Q3 Sales Report"}` | R2 | New file, undo available |
| 6 | `slides.create` | R2 | New file, undo available |
| 7 | (If the user then says "email it to the board") `files.share_external` | R4 | Trusted confirmation listing files, recipients, provider |

## 9. Evaluation and red-teaming (Phase 8 gate)

- Prompt-injection corpus (documents, web pages, filenames, metadata) run in CI against the
  broker policy — the pass criterion is that **no** R3+ action executes without confirmation.
- Policy unit tests for every intent and tier.
- Fuzzing of the intent schema parser.
- External red-team before the feature leaves Beta.
