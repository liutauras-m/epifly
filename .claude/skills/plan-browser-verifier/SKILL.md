---
name: plan-browser-verifier
description: Use this skill whenever implementing a multi-phase plan, roadmap, ticket, or feature that produces user-visible UI or HTTP behaviour. After EACH phase/feature/checkbox is coded, this skill drives a real browser (Claude in Chrome MCP, Claude Preview MCP, or Playwright/curl fallback) to exercise the new functionality end-to-end, captures screenshots + console + network logs as evidence, and audits the UI against a concrete checklist of modern UI/UX best practices (visual hierarchy, contrast/WCAG AA, spacing rhythm, typography, motion, responsive layout, error/empty/loading states, keyboard + focus). Invoke proactively — do not wait for the user to ask "did you test it?". Triggers include: "implement the plan", "start phase N", "build this feature", "follow plan.md", "next step in roadmap", or any time a TODO list with implementation steps is being executed against a web frontend.
---

# Plan Browser Verifier

Implementation without verification is a guess. This skill turns every phase of a plan into a closed loop: **code → run → drive in a real browser → capture evidence → audit UI quality → only then mark complete**.

## When this skill runs

You are implementing something from a plan, roadmap, or TODO list — anything where work proceeds in discrete phases or features and the output is reachable via a browser (HTML page, SSE endpoint, JSON API, file upload flow, etc.).

The trigger is structural, not lexical: if there is a list of steps and you just finished one, run the verification loop before starting the next.

## The loop (run after every phase)

For each completed phase, do these five steps in order. Do not skip a step because "it's obvious" — the discipline is the value.

### 1. State what you just built and what to verify

Write 1–3 sentences in chat:
- What changed (file paths + the user-visible behaviour)
- The exact thing a user should now be able to do
- The URL / endpoint / button that exercises it

This forces a concrete acceptance criterion before the test, not after.

### 2. Make sure the thing is running

Check the dev server / docker compose / binary is up. If it isn't, start it and wait for the health endpoint. Don't try to verify against a stale build — confirm the binary or bundle reflects your new code (timestamp check, version endpoint, or trivial known-changed string in the response).

### 3. Drive a real browser

Pick the highest-fidelity tool available, in this order:

1. **`mcp__Claude_in_Chrome__*`** — full browser automation, real DOM, real network. Use `navigate`, `read_page` / `get_page_text`, `find`, `computer` (click/type), `read_console_messages`, `read_network_requests`. Best for anything interactive.
2. **`mcp__Claude_Preview__*`** — preview server with screenshots, console, eval. Use `preview_start`, `preview_screenshot`, `preview_console_logs`, `preview_eval`, `preview_click`, `preview_fill`. Best for static/component previews.
3. **Headless fallback** — `curl -i` for HTTP, plus a screenshot via `chromium --headless --screenshot=...` or Playwright if installed. Use only when no MCP browser is available.

Walk the **golden path** (the happy case the phase was built for) and at least one **edge** (empty input, invalid input, large input, slow network, or a permission/tenant boundary if multitenant).

Capture for each run:
- A screenshot of the relevant viewport
- The URL + HTTP status
- Console errors/warnings (zero is the bar — investigate any that appear)
- Network requests for the action (status, latency, payload size)

### 4. Audit the UI against the checklist

Only run this for phases that produced or changed UI. Skim the rendered page and the screenshot and check each item. Note any violation explicitly — "passes" with no notes is fine.

**Visual hierarchy & layout**
- One clear primary action per view; secondary actions visually subordinate
- Consistent spacing rhythm (4 / 8 / 16 / 24 / 32 — or whatever the design tokens define); no ad-hoc pixel values
- Alignment to a grid; no orphaned elements floating off-axis
- Whitespace breathes — content is not crammed edge-to-edge

**Typography**
- ≤ 2 type families; ≤ 5 sizes in use on the page
- Line height ≥ 1.4 for body text; line length 45–80 chars
- Numerals and labels not shouting (avoid all-caps walls, gratuitous bold)

**Colour & contrast**
- Body text contrast ≥ 4.5:1 (WCAG AA); large text ≥ 3:1; UI components ≥ 3:1
- Accent colour used sparingly — primary CTA and selection only, not decoration
- Light and dark themes both pass contrast (test both if both exist)

**Interactive states**
- Every interactive element has hover, focus-visible, active, and disabled states
- Focus ring is visible against all backgrounds (don't remove the outline without a replacement)
- Tap targets ≥ 40×40px on touch contexts

**Feedback & state coverage**
- Loading state shown for any async action > 200ms
- Empty state has guidance, not just a blank panel
- Error state is human-readable, recoverable, and tied to the field that caused it
- Success state is acknowledged (toast, inline, or state change) — never silent

**Motion**
- Durations 120–300ms for micro-interactions; easing is not linear
- Respects `prefers-reduced-motion` (no parallax / large transforms when set)
- Nothing strobes or auto-advances faster than the user can read

**Responsive & accessibility**
- Layout works at 360px, 768px, 1280px without horizontal scroll
- Keyboard-only flow can complete the golden path (Tab order is sane)
- Images have alt text; icons used as buttons have aria-labels
- Form fields have associated `<label>`s, not just placeholders

**Performance smell test**
- First meaningful paint feels under ~1s on localhost
- No layout shift after initial render (CLS ≈ 0)
- No console errors, no 404s on assets, no oversized images (>500KB warrants a look)

### 5. Report and gate

Write a short verdict block before moving on:

```
Phase N verification — <pass | pass-with-notes | fail>
- Built: <one line>
- Verified at: <URL>
- Golden path: <pass | fail — what happened>
- Edge case (<which>): <pass | fail — what happened>
- UI audit: <pass | N findings — list them tersely>
- Evidence: <screenshot path(s), key console/network notes>
```

If anything is `fail` or has UI findings worth fixing, fix them **before** starting the next phase. Don't accumulate debt across phases — each phase ships clean or doesn't ship.

If the user is in auto mode, fix routine findings (contrast tweaks, missing focus rings, missing aria-labels, missing loading state) without asking. Only stop and ask for genuinely ambiguous design calls (e.g. "should this be a modal or a drawer?").

## Tool selection cheat sheet

| Need | Reach for |
|---|---|
| Click / type / submit form | `mcp__Claude_in_Chrome__computer` or `preview_click` / `preview_fill` |
| Read what's on screen | `mcp__Claude_in_Chrome__get_page_text` or `read_page` |
| Console errors | `read_console_messages` / `preview_console_logs` |
| Network calls fired | `read_network_requests` / `preview_network` |
| Screenshot evidence | `mcp__Claude_in_Chrome__gif_creator` (sequence) or `preview_screenshot` |
| Pure JSON endpoint | `curl -sS -i -H 'X-Tenant-ID: dev' …` |
| Multi-viewport check | `resize_window` then re-screenshot at 360 / 768 / 1280 |

If no browser MCP is connected, ask the user to enable Claude in Chrome or Claude Preview *once* at the start of the implementation session, not on every phase.

## What this skill is not

- Not a unit-test runner. `cargo test` / `npm test` is still the user's responsibility — this skill exists because tests don't catch "the button is invisible on dark mode" or "the SSE stream silently 500s".
- Not a design system author. It audits against generally-accepted UI heuristics; if the project has its own design tokens / guidelines, prefer those and treat this checklist as the floor.
- Not exhaustive accessibility certification. WCAG AA contrast + keyboard + alt text is the bar; full a11y audits are a separate engagement.

## Failure modes to avoid

- **Marking a phase done because the code compiled.** Compilation is necessary, not sufficient. The loop above is the gate.
- **Verifying against a cached / stale server.** Always confirm the running binary reflects the code change.
- **Screenshots without commentary.** A screenshot proves nothing on its own — say what it shows and what you concluded.
- **"It looked fine to me" UI audits.** Walk the checklist explicitly. Each item gets a one-word verdict.
- **Hiding console errors.** Any red in the console is a finding. Fix it or document why it's expected.
