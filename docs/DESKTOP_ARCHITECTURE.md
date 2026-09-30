# NUR OS — Desktop Architecture, Design System, Internationalisation, Accessibility

Status: **Wayland-first is DECIDED** ([ADR-0004](adr/0004-wayland-first-desktop.md)); compositor
foundation **PROPOSED** ([ADR-0005](adr/0005-compositor-foundation.md)); UI toolkit **RESEARCH
REQUIRED** ([ADR-0006](adr/0006-ui-toolkit.md)). No desktop code exists.

## 1. Goals

- Responsive: frame-perfect animations on modest integrated GPUs; input latency tracked per release.
- Robust: a crash in any shell component must not end the session.
- Secure: the compositor is the enforcement point for screen capture, input capture, clipboard
  focus rules, and trusted prompts.
- Independent identity: NUR's own interaction model and visual language, not a Windows/macOS copy.
- Replaceable: applications never depend on compositor/shell internals.

## 2. Compositor options

| Option | Language | License | Pros | Cons |
| --- | --- | --- | --- | --- |
| **Smithay (library) + NUR compositor** | Rust | MIT | Memory-safe; permissive license; proven in production by COSMIC (cosmic-comp) and niri; full control of security/UX | We own a large, critical component; multi-GPU, HDR, VRR, colour management require sustained investment |
| wlroots + NUR compositor | C | MIT | Mature, widely used (Sway, many others) | C in the most exposed userspace component; conflicts with memory-safety preference |
| Fork/extend cosmic-comp | Rust | GPL-3.0-only | Fastest route to a complete Rust desktop compositor | GPL-3.0 constrains future licensing of linked code; tight coupling to COSMIC design and roadmap |
| KWin (KDE Plasma) | C++ | GPL-2.0-or-later | Very feature-complete (HDR, VRR, fractional scaling, accessibility) | Tied to Qt/KDE Frameworks; large; heavy coupling to Plasma shell concepts |
| Mutter (GNOME Shell) | C / JS | GPL-2.0-or-later | Mature, strong a11y | Shell logic in JavaScript inside the compositor process; tightly coupled to GNOME |

**Recommendation (PROPOSED):** build `nur-compositor` on **Smithay**. It matches the Rust and
permissive-licence preferences and has production evidence. The risk is scope: Phase 3 must deliver a
deliberately limited compositor (single-GPU, SDR, integer + fractional scaling, multi-monitor,
XWayland) and defer HDR/VRR/colour management to later phases.

**Decision requiring approval:** whether to accept the engineering cost of an owned compositor
versus adopting KWin or cosmic-comp as a base. See [RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md).

## 3. Process architecture

```text
nur-session (systemd user target: nur-session.target)
├── nur-compositor            (Wayland server, DRM/KMS, libinput, XWayland on demand)
│    ├── enforces: screencast/input-capture portals, clipboard focus, trusted-prompt layer
│    └── NUR private protocols (nur_shell_v1, nur_prompt_v1) — only for shell clients
├── nur-shell-panel           (dock/taskbar, tray, clock)          — layer-shell client
├── nur-shell-launcher        (app grid, search)                   — on demand
├── nur-shell-notifications   (org.freedesktop.Notifications)      — D-Bus activated
├── nur-shell-quicksettings   (network, audio, power, BT toggles)  — on demand
├── nur-shell-prompter        (permission / polkit / AI confirmation prompts) — trusted
├── nur-lockscreen            (ext-session-lock-v1)                 — on lock
├── xdg-desktop-portal + xdg-desktop-portal-nur
└── apps (each in its own systemd scope under app.slice)
```

- Components use **standard protocols first** (`xdg-shell`, `wlr-layer-shell`-equivalent,
  `ext-session-lock-v1`, `xdg-activation`, `fractional-scale-v1`, `text-input-v3`,
  `input-method`, `ext-image-capture-source`/screencopy equivalents only for trusted clients).
- `nur-shell-prompter` is the only client allowed to use `nur_prompt_v1`, which the compositor
  renders on a layer above all application surfaces, blocks synthetic input to it, and marks
  with a secure indicator region that applications cannot draw.
- If a shell component crashes, systemd restarts it; the compositor keeps windows alive.
- Compositor crash currently ends the session. Research item: client session-restore via
  upstream Wayland efforts for compositor handoff/restart; until then the compositor has the
  highest test and fuzzing bar in the desktop.

## 4. Feature architecture

| Area | Design notes | Phase |
| --- | --- | --- |
| Window management | Stacking by default with snapping/tiling assists; workspaces; keyboard-first | 3 |
| Dock/taskbar, launcher | Separate clients; launcher search via `org.nur.Search1` (apps, settings, files — indexer research) | 3 |
| Notifications | freedesktop Notifications spec + portal; per-app policy; Do Not Disturb | 3 |
| Quick settings | Talks to NUR APIs, never directly to NetworkManager internals | 3 |
| Lock screen / greeter | `ext-session-lock-v1`; greeter as separate minimal session | 3 |
| Settings | Modular panels; each panel talks to one `org.nur.*` API | 3 → 7 |
| Screenshots / recording | Compositor-native capture for the user's own action, portal for apps | 3/7 |
| Clipboard manager | Opt-in history, excludes password-hinted content (`x-kde-passwordManagerHint`-style MIME hints), clears on lock option | 7 |
| Display management | Multi-monitor, per-output scale, fractional scaling, rotation, mirroring | 3/4 |
| HiDPI | Fractional scaling via `wp-fractional-scale-v1` + viewporter; XWayland scaling policy research | 3 |
| Input | libinput; touchpad gestures; keyboard layouts via xkbcommon; tablet support later | 3/4 |
| Networking / BT / audio / power UI | Quick settings + Settings panels over NUR APIs | 3/4 |
| Accessibility services | Screen reader (Orca initially), magnifier, high contrast, reduced motion, sticky/slow keys, on-screen keyboard | 3 → 7 |
| HDR / VRR / colour management | Deferred; upstream Wayland colour-management protocol | 11+ |

## 5. NUR Design System (to be produced in Phase 3)

Deliverables: design tokens (data files, toolkit-independent), component specifications,
interaction guidelines, icon set, typography, motion spec, accessibility checklist.

| Topic | Rule set to define |
| --- | --- |
| Typography | Type scale in logical units; scripts coverage (Latin, Cyrillic, Arabic, CJK fallback via Noto); minimum body size |
| Spacing & layout | 4-px base grid in logical pixels; adaptive layouts for narrow windows/tablets |
| Colour | Semantic tokens (surface, accent, danger...) with light/dark/high-contrast variants; WCAG 2.2 AA contrast minimum |
| Components | Buttons, lists, dialogs, sheets, toasts, menus: states for hover/focus/pressed/disabled/error/loading |
| Dialogs | Title states the question; primary action names the verb ("Delete 3 files"), never "OK" for destructive actions |
| Destructive actions | Confirmation proportional to reversibility; prefer undo (trash, snapshots) over confirmation |
| Navigation | Consistent back/escape semantics; full keyboard reachability; visible focus rings |
| Windows | Client-side vs server-side decorations policy (research; prefer SSD for consistency, support CSD) |
| Notifications | Urgency levels; no notification for routine success unless user-initiated |
| Motion | Short, interruptible, respects reduced-motion; no animation blocks input |
| Touch targets | ≥ 44×44 logical px equivalent for touch-capable form factors |
| Error / empty / loading states | Every view defines all three; errors state cause and next action |
| Icons | Original NUR icon set; no reuse of Windows/macOS assets |

## 6. Internationalisation

| Requirement | Design |
| --- | --- |
| Encoding | UTF-8 only |
| Text rendering | HarfBuzz shaping, FreeType rasterisation, Fontconfig fallback chains per script |
| Bidirectional text | Toolkit must support BiDi and mirrored layouts; RTL pseudo-locale in CI |
| Formatting | Dates, times, numbers, currencies, units, list formatting from CLDR (ICU/ICU4X) |
| Plurals / gender | Message format with CLDR plural rules (Fluent or ICU MessageFormat — decision pending) |
| Catalogues | Strings extracted from code; translation platform (Weblate or similar, self-hosted — research) |
| Pseudo-localisation | CI renders UI with expanded (+40 %) and RTL pseudo-locales to catch truncation and hard-coded strings |
| Keyboard layouts | xkeyboard-config; Uzbek Latin/Cyrillic, Russian, English at launch; per-window or global switching |
| Input methods | IBus or Fcitx5 via Wayland input-method protocol (research) |
| Locale selection | Per-user language, region format independent of language |
| Initial locales | uz-Latn, en, ru — no code path may special-case them |

## 7. Accessibility

- AT-SPI2 exposure for all first-party UI; toolkit choice weighs this heavily.
- Keyboard-only operation of the entire desktop and every first-party app.
- Screen reader, magnifier, high-contrast theme, large text, reduced motion, sticky/slow/bounce
  keys, cursor size, on-screen keyboard.
- Accessibility permission (reading other apps' UI, injecting input) is a sensitive permission
  granted only to assistive technologies with explicit user action
  ([SECURITY_MODEL.md](SECURITY_MODEL.md)).
- Release gate: automated a11y tree checks + manual screen-reader walkthroughs for every
  first-party app ([TESTING_STRATEGY.md](TESTING_STRATEGY.md)).
