# UI Landmark Map

**Source of truth for ARIA landmark roles across all routes.**  
Companion docs: [`docs/ui-design.md`](ui-design.md) §11 (focus & a11y), [`ui-plan.md`](ui-plan.md) §8.1 (axe CI gate).

Auditors reference this during Phase 3+ reviews. Updated at Phase 8.4 with per-route axe reports.

---

## Landmark rules (invariants)

| Landmark          | Count per page | Component       | Notes |
|-------------------|---------------|-----------------|-------|
| `banner`          | Exactly 1     | `AppHeader`     | The `<header role="banner">` inside `AppShell`'s topbar slot |
| `main`            | Exactly 1     | `AppShell`      | `<main>` in the shell-main slot |
| `navigation`      | 0–1           | `Sidebar`       | Default role for sidebar. `aria-label="Workspace"` |
| `complementary`   | 0–1           | `Sidebar`       | Use when sidebar holds supplemental content (detail panels) |
| `form`            | 0–1 on chat   | `AppShell`      | `<form aria-label="Message composer">` — chat routes only |
| `search`          | 0–1           | `SidebarSearch` | The ONE legitimate `role="search"` consumer. Never on Composer. |
| `contentinfo`     | 0–1           | (future footer) | Reserved for footer/copyright. None currently. |

**Rules:**
- `banner` + `main` must appear **exactly once** on every page — axe CI gate enforces this.
- `form[aria-label="Message composer"]` renders **only on chat routes** (`/` and `/chat/*`).
- `role="search"` belongs **only on actual search/filter inputs** — the SidebarSearch field. Never the Composer.
- No landmark may be emitted from an empty slot (guard each slot with `{#if …}`).

---

## Per-route landmark matrix

| Route / screen            | `banner` | `navigation` | `complementary` | `main` | `form` (composer) | `search` |
|---------------------------|----------|--------------|-----------------|--------|--------------------|----------|
| `/` (greeting / chat)     | ✓        | ✓ "Workspace" | —               | ✓      | ✓ "Message composer" | ✓ (sidebar) |
| `/chat/:id` (active chat) | ✓        | ✓ "Workspace" | —               | ✓      | ✓ "Message composer" | ✓ (sidebar) |
| `/account/**`             | ✓        | ✓ "Workspace" | —               | ✓      | —                  | ✓ (sidebar) |
| `/login`                  | —        | —             | —               | ✓      | ✓ "Sign in"*       | — |
| Future: split-view detail | ✓        | ✓ "Workspace" | ✓ "Details"     | ✓      | —                  | ✓ (sidebar) |

\* The login form uses `<form aria-label="Sign in">` — a different label from the composer. Only one `role="form"` landmark per page.

---

## `AppShell` sidebar role selection

```svelte
<!-- Most routes — sidebar is primary navigation -->
<AppShell sidebarRole="navigation">…</AppShell>

<!-- Future: screen with right supplemental panel -->
<AppShell sidebarRole="complementary">…</AppShell>

<!-- Login / no sidebar -->
<AppShell>…</AppShell>  <!-- no sidebar slot → no landmark emitted -->
```

---

## Verification checklist (per PR)

- [ ] Open axe DevTools → Landmarks tab. Verify count per column above.
- [ ] iOS VoiceOver: swipe to Rotor → Landmarks → enumerate. Match the matrix.
- [ ] `banner` and `main` exist exactly once (axe CI rule `landmark-one-main`, `landmark-banner-is-top-level`).
- [ ] No `role="search"` on the Composer — verify via DOM inspector on `/`.
- [ ] `form[aria-label="Message composer"]` present on `/` and `/chat/:id`, absent on `/account/**` and `/login`.
