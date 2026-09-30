# NUR OS — Application Platform, Permissions, Compatibility, and NUR Store

Status: **PROPOSED** ([ADR-0009](adr/0009-application-packaging.md),
[ADR-0010](adr/0010-sandbox-and-permission-model.md)). Nothing here is implemented.

## 1. Application classes

| Class | Examples | Delivery | Sandbox | Trust |
| --- | --- | --- | --- | --- |
| **System components** | Compositor, shell, Settings, Update UI, Security Center, portal backend | Part of the signed OS image | Hardened systemd units / MAC; not Flatpak (they *implement* the trusted path) | Trusted, minimal set, security-reviewed |
| **First-party apps** | Files, Terminal, Text Editor, Office, AI app | Flatpak built against NUR runtime, published in NUR Store (some preinstalled) | Yes — same rules as third-party, no silent privileges | High, but not TCB |
| **Third-party native apps** | Apps built on NUR SDK | NUR Store (Flatpak) | Yes | Per-developer verification |
| **Linux apps** | Existing Flatpaks (e.g. from Flathub if user enables that remote) | Flatpak remote, clearly labelled third-party | Yes (manifest-declared, may be broad) | Per-remote policy |
| **Web apps / PWAs** | Web services installed from browser | Browser-managed | Browser sandbox | Web origin model |
| **Developer workloads** | Toolchains, databases, CLI tools | Podman containers (Toolbx/Distrobox) | Container isolation; shares `$HOME` by default in Toolbx (documented risk) | User-managed, outside TCB |
| **Windows apps** | Legacy Windows software, games | Wine/Proton inside a Flatpak sandbox | Yes | Untrusted; no promise of compatibility |

The host OS is immutable: **end users do not install traditional packages into `/usr`.** CLI
tools go into containers; GUI apps into Flatpak.

## 2. Why Flatpak (summary of ADR-0009)

- Mature, widely adopted, cross-distro format with a large existing catalogue.
- Sandbox built on bubblewrap + seccomp; portal-based access model aligned with ours.
- Runtimes decouple apps from the OS version, which fits an image-based OS.
- Upstream license LGPL-2.1-or-later; format is open.

Known weaknesses we must engineer around:

| Weakness | NUR response |
| --- | --- |
| Static permissions in manifests can be over-broad (`filesystem=home`, `--talk-name=org.freedesktop.Flatpak`, `--socket=x11`) | Store policy rejects or flags; install dialog shows risk-ranked permissions; users can override (`flatpak override`) via Settings UI |
| Network is all-or-nothing | Per-app network control research ([SECURITY_MODEL.md](SECURITY_MODEL.md)) |
| Repository signing is GPG over OSTree | NUR Store adds TUF-signed Store metadata binding app ID → developer key → commit checksums (§6) |
| Large runtimes on disk | Shared NUR runtime; deduplication via OSTree |

The **NUR Store manifest is format-independent**: if Flatpak is ever replaced, the Store identity,
permission, and trust model remain.

## 3. Permission model

### 3.1 Principles

1. Deny by default; request at the moment of need, with a purpose string supplied by the app and
   shown by the system.
2. Grants are bound to the **app identity** (app ID + developer signing key), not to a binary path.
3. Grants are revocable at any time in Settings → Privacy & Security; revocation takes effect
   immediately (running streams are cut).
4. Sensitive resource use is **visible** (persistent indicators for mic, camera, screen capture,
   accessibility).
5. Prompts are rendered by the shell on the trusted path; apps cannot draw prompts that look like
   system prompts over fullscreen content.
6. Every grant, denial, and revocation is audit-logged.
7. Enterprise/parental policy can pre-deny or pre-grant within limits defined by the user-facing
   model (no invisible grants of mic/camera/screen).

### 3.2 Permission catalogue

The canonical table (defaults, grant options, enforcement, indicators) is in
[SECURITY_MODEL.md §7](SECURITY_MODEL.md#7-permission-model-summary). This document owns the
lifecycle and API.

### 3.3 Lifecycle

```text
App calls portal (e.g. org.freedesktop.portal.Camera.AccessCamera)
  → xdg-desktop-portal → xdg-desktop-portal-nur backend
      → nur-permissiond: lookup(app_id, permission)
          granted  → return handle / fd (PipeWire node, document fd)
          denied   → return error (no re-prompt spam: rate-limited)
          unknown  → ask shell to render trusted prompt → store decision → audit
```

- Permission storage: `nur-permissiond` backs the upstream permission store API so standard
  portals keep working, and adds NUR metadata (grant scope, expiry, last use).
- "Once" grants expire when the app's instance exits. "While running" grants expire when the app
  scope ends. "Always" grants are reviewed: Security Center shows unused grants older than
  90 days and suggests revocation (auto-revocation policy: research).

### 3.4 Platform API for apps (to be specified in Phase 5)

`org.nur.Permissions1` (session bus, app-facing through the portal):

- `QueryStatus(permission) → state` — so apps can explain before asking;
- `Request(permission, purpose_id) → result` — purpose_id refers to a localised string in the
  app's manifest (not free text), to stop apps inserting misleading prompt text;
- change notifications when the user revokes.

## 4. Compatibility strategy

| Target | Approach | Promise |
| --- | --- | --- |
| Linux GUI apps | Flatpak | Works if the app works in Flatpak; tested catalogue subset listed in Store |
| Linux CLI / dev tools | Podman + Toolbx/Distrobox with common distro images | Standard container behaviour |
| Web apps | Standards-compliant browser; PWA install integration with launcher | Browser-dependent |
| Android apps | Not planned. Research candidates (e.g. Waydroid) only after 1.0 | None |
| Windows apps | Wine/Proton in a dedicated Flatpak sandbox with per-prefix isolation; curated compatibility list with test evidence | **No universal compatibility.** Only listed apps at listed ratings |

Legal note: Wine is LGPL-2.1-or-later and is a clean-room implementation; NUR never ships
Microsoft binaries (DLLs, fonts, redistributables) without a license. Anti-cheat and DRM-protected
software is expected not to work.

## 5. NUR SDK (outline)

- NUR runtime + SDK as Flatpak runtimes, versioned per NUR platform release, with ≥ 2 years of
  security support per runtime version.
- Language bindings: Rust first; others driven by the toolkit decision.
- Includes: toolkit, design-system components, i18n libraries, portal client libraries,
  `org.nur.*` API clients, AI Intent API client (Phase 8).
- API stability policy: semantic versioning per interface; deprecation ≥ one Stable cycle.

## 6. NUR Store architecture (design only; no marketplace in Phase 0)

### 6.1 Identity and trust

- **App ID**: reverse-DNS (`org.example.App`), globally unique in the Store.
- **Developer identity**: verified account with a registered signing key (hardware-backed
  recommended). App ID ownership is bound to the developer key; transfer requires Store-mediated
  re-keying.
- **Store metadata** signed with TUF roles (same structure as OS updates, separate keys).

### 6.2 Manifest (draft schema, illustrative)

```yaml
schema: nur.store.manifest/v0
app_id: org.example.Notes
version: 1.4.2               # semver
build: 142                   # monotonic
developer:
  id: dev-7f3a...
  key_fingerprint: sha256:...
package:
  format: flatpak
  ref: app/org.example.Notes/x86_64/stable
  commit: <ostree commit checksum>
  runtime: org.nur.Platform//1
  arches: [x86_64, aarch64]
permissions:                 # must be a superset of the Flatpak finish-args after normalisation
  - id: files.documents
    access: read-write
    purpose: purpose.sync_notes      # key into localised strings
  - id: network
  - id: notifications
sandbox:
  x11: false
  host_filesystem: false
content_rating: ...
privacy:
  data_collected: []
  policy_url: https://example.org/privacy
min_os_version: "1.0"
signatures: [...]            # detached; verified against developer key and Store targets role
```

### 6.3 Pipeline

```text
Developer upload (signed)
 → automated checks: manifest schema, permission normalisation (manifest vs finish-args),
   static analysis, malware scanning, license declaration, SBOM presence
 → risk scoring (broad permissions, X11, network + files, accessibility)
 → human review for high-risk or first submission
 → build reproduction (where the source is provided) on Store infrastructure
 → Store signs targets metadata → staged rollout → monitoring & takedown capability
```

### 6.4 Lifecycle operations

| Operation | Behaviour |
| --- | --- |
| Install | Verify TUF metadata → verify developer signature → verify commit → show permissions → install |
| Update | Permission *increases* require user re-consent; decreases are silent |
| Rollback | Previous commit retained (configurable); user can revert one version |
| Uninstall | Remove app; offer to delete app data (`~/.var/app/<id>`), default keep with clear size shown |
| Takedown | Store marks version/app revoked; clients warn and offer removal; no silent remote deletion of user data |

### 6.5 Not in scope before Phase 6

Payments, ratings/reviews, developer revenue, enterprise catalogues.
