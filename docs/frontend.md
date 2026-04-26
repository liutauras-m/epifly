# ConusAI Frontend Implementation Plan

Last updated: 2026-04-26
Status: ready to build

A production-grade chat UI that becomes the canonical entrypoint for ConusAI.
Implemented inside `crates/agent-gateway` so the platform ships as a single
binary. Structural cues are taken from `docs/ui.png` (Claude reference);
visual identity is **distinct, opinionated, and ours**.

---

## 1. Purpose & Audience

| Question | Answer |
|---|---|
| **Who uses this?** | ConusAI operators (engineers, analysts) running an agent platform — long sessions, dense screens, mixed code/text/file workflows. |
| **What problem does it solve?** | A single canonical UI to drive every backend capability (chat, tools, threads, files, capabilities, evals) without bespoke clients per surface. |
| **What must be unforgettable?** | The **first 800ms**: the cream-paper composer rising under a serif greeting, the ember sigil pulse, the low industrial seam down the sidebar. Users should remember the *temperature* of the interface. |

---

## 2. Aesthetic Direction — "Foundry"

We commit to one direction and execute it with precision.

> **Foundry** — editorial-industrial. The warmth of letterpress paper meeting
> the precision of machined steel. Confident serif headlines paired with a
> geometric grotesque body and ligature-rich monospace for code. One
> dominant accent (ember/copper) on a cream-paper or carbon-ink ground.
> Hairline rules instead of boxes. Generous negative space.
> Type-led, not chrome-led.

### Why this works for ConusAI
- The agent platform is a **workshop**, not a chatbot toy. Foundry signals craft.
- Editorial typography rewards **dense reading** (long agent traces, JSON tool calls).
- One dominant accent makes streaming + tool-call states legible without color noise.
- Honours `docs/ui.png` structurally (sidebar + greeting + composer + chips) without copying its surface.

### Anti-patterns we will not ship
- Purple-pink gradients on white. Glassmorphism. Generic Inter/Roboto.
- Rounded-everything 12px-radius card stacks. Centered hero with a "Get started" button.
- Emoji greetings. Unicode `✳` placeholder marks. Lucide icons used raw without sizing pass.

---

## 3. Design System

### 3.1 Color tokens

Two themes share the same accent and structural tokens. Default is **Paper** (matches `docs/ui.png`); **Forge** is the dark counterpart, toggled by user.

```css
/* Paper (default, matches ui.png) */
--ink:        #14110D;   /* primary text, near-black with warm undertone */
--ink-2:      #3A332B;   /* secondary text */
--ink-3:      #6E6357;   /* tertiary text, muted labels */
--paper:      #F4EEE3;   /* page bg — warm cream, not white */
--paper-2:    #EBE3D4;   /* sidebar bg, raised cards */
--paper-3:    #DFD4BF;   /* hover surfaces */
--rule:       #D6CAB0;   /* hairline borders, 1px */
--seam:       #C2B391;   /* stronger divider */

/* Forge (dark) */
--ink:        #F4EEE3;   /* invert: text becomes paper */
--ink-2:      #C8BFAE;
--ink-3:      #8A8174;
--paper:      #100E0B;   /* carbon black, warm */
--paper-2:    #181612;   /* sidebar */
--paper-3:    #211E18;
--rule:       #2A251E;
--seam:       #3A3328;

/* Shared accents (both themes) */
--ember:      #D9531E;   /* primary accent — flame copper */
--ember-2:    #B33F12;   /* pressed / shadow */
--ember-soft: rgba(217, 83, 30, 0.10);
--steel:      #5C6B7A;   /* neutral accent (link, info) */
--rust:       #8B2E0E;   /* error / destructive */
--moss:       #4A6B3A;   /* success / running tool */

/* Streaming token glow (sparingly) */
--cursor:     linear-gradient(180deg, transparent 0%, var(--ember) 50%, transparent 100%);
```

### 3.2 Typography

```css
/* Display / brand — Fraunces (variable, wedge serifs, optical sizing) */
--font-display: "Fraunces", "Tiempos Headline", "Iowan Old Style", Georgia, serif;
font-variation-settings: "opsz" 96, "SOFT" 30, "WONK" 0, "wght" 400;

/* Body — Switzer (Fontshare, geometric grotesque, free) */
--font-body: "Switzer", "Söhne", "Inter Tight", system-ui, sans-serif;

/* Mono — JetBrains Mono (ligatures, distinctive) */
--font-mono: "JetBrains Mono", "Berkeley Mono", "IBM Plex Mono", ui-monospace, monospace;
```

Type scale (modular, ratio 1.2):
| Token | Size / line | Usage |
|---|---|---|
| `--t-display` | 48 / 56 | greeting |
| `--t-h1` | 28 / 36 | section titles |
| `--t-h2` | 20 / 28 | message headers |
| `--t-body` | 15 / 24 | chat copy |
| `--t-meta` | 13 / 20 | timestamps, metadata |
| `--t-label` | 11 / 16 | uppercase mono labels (`tracking: 0.14em`) |
| `--t-mono` | 13 / 20 | code, tool JSON |

Greeting uses `font-variation-settings: "opsz" 96` to engage Fraunces' display optical size — distinctive wedge serifs that small-text Inter clones cannot reproduce.

### 3.3 Spacing & rhythm

```css
--s-1: 4px; --s-2: 8px; --s-3: 12px; --s-4: 16px;
--s-5: 24px; --s-6: 32px; --s-7: 48px; --s-8: 72px;
--rail: 260px;       /* sidebar width */
--gutter: 64px;      /* main column inset on desktop */
--composer-w: 720px; /* max input width */
```

### 3.4 Motion

```css
--ease-out: cubic-bezier(0.2, 0.8, 0.2, 1);
--ease-in:  cubic-bezier(0.6, 0, 0.7, 0.2);
--dur-1: 120ms; --dur-2: 200ms; --dur-3: 320ms; --dur-4: 520ms;
```

**Page-load orchestration** (the unforgettable 800ms):
1. `0ms` — sidebar rail draws in (1px ember stroke, left-to-right, 320ms `ease-out`)
2. `120ms` — sidebar items cascade in (16ms stagger, opacity + 4px `translateY`)
3. `420ms` — greeting fades in with serif set in display optical size, ember sigil pulses once (single 600ms `breath` keyframe)
4. `560ms` — composer rises from 8px below, soft inner shadow settles
5. `680ms` — quick chips reveal left-to-right (40ms stagger)

**Streaming**: each token block fades in over 80ms with `translateY(2px) → 0`. A 2px-wide ember vertical bar pulses at the cursor position (1.2s `breath` loop).

**Reduced motion** (`@media (prefers-reduced-motion: reduce)`): all transforms removed, durations clamped to 80ms, opacity-only.

### 3.5 Background & atmosphere

- Subtle radial vignette: `radial-gradient(ellipse at 18% 8%, var(--ember-soft), transparent 45%)` fixed to viewport. Adds warmth to top-left without competing with content.
- 2.5% noise overlay (SVG `feTurbulence`) on `body::after` with `mix-blend-mode: overlay` — adds paper grain without blurring text.
- 1px hairline rules instead of card borders — `border-bottom: 1px solid var(--rule)`. No box-shadows on cards; depth comes from typography weight and rules.

### 3.6 Iconography

Hand-tuned 18×18 SVG sprite (single file `assets/icons.svg`, referenced via `<use>`). 1.5px stroke weight, square caps, no rounded corners on technical icons (compass for capabilities, square for files, slash for code) to reinforce the "instrument" feel. Decorative marks (sigil, brand monogram) use Fraunces glyph fragments.

**Brand sigil**: a hand-drawn ember spark (custom SVG, 5 strokes, 32×32) used beside greeting and as favicon. Replaces the unicode `✳` in the reference.

---

## 4. Component Inventory

| Component | File | Notes |
|---|---|---|
| `Sidebar` | `partials/sidebar.html` | rail (260px), brand monogram, primary nav, recents, **capabilities** group, user chip |
| `Greeting` | `partials/greeting.html` | time-adaptive ("Morning" / "Afternoon" / "Evening" + name), sigil, ember pulse |
| `Composer` | `partials/composer.html` | textarea, attach button, model selector, submit, attachments preview row |
| `QuickChips` | `partials/quick_chips.html` | Code · Write · Learn · Life stuff · Operator's choice |
| `MessageUser` | `partials/message_user.html` | right-aligned, ember left-edge accent, attachments below |
| `MessageAI` | `partials/message_ai.html` | full-width, serif drop-cap on session-opening message, mono for inline code |
| `ToolCard` | `partials/tool_card.html` | mono header (`◆ tool_name · capability · 23ms`), collapsible JSON body, status dot |
| `Attachment` | `partials/attachment.html` | thumbnail (image/pdf/audio glyph), filename, size, remove |
| `CapabilityCard` | `partials/capability_card.html` | sidebar item: kind glyph + name + tool count, click → `@capability` mention in composer |
| `Login` | `templates/login.html` | split layout: left brand panel (ember gradient + monogram), right form (name field, "Enter the workshop") |

---

## 5. Tech Stack

| Layer | Choice | Rationale |
|---|---|---|
| Templates | **Askama 0.12** | compile-time, type-safe, already in workspace deps |
| Routing | **Axum 0.8** | already in gateway |
| Cookies / multipart | **axum-extra 0.10** (`cookie`, `multipart`) | small surface |
| Reactivity | **Alpine.js 3.x** (CDN) | ephemeral UI state (chip toggle, attachment preview, theme switch) without a build step |
| Server-driven UX | **HTMX 2.0** (CDN) | progressive enhancement for non-streaming partials (load older messages, list threads) |
| Streaming | **Fetch + ReadableStream** (vanilla) | SSE → DOM token append; HTMX SSE plugin can't render mid-message tool cards naturally |
| **Styling** | **Hand-crafted CSS** (~600 lines, one file) | refined design needs tight control; Tailwind utility soup undermines editorial typography. No build step. |
| Fonts | Fraunces + Switzer + JetBrains Mono | served from Google Fonts (Fraunces, JetBrains) + Fontshare (Switzer). All free. |
| Icons | hand-tuned SVG sprite | one HTTP call, sized for our type scale |

**Rejected**: Tailwind (utility soup conflicts with editorial typography), React/Vite (single-binary requirement), shadcn (generic look), Bootstrap (predictable).

**Static assets**: served via `tower-http::services::ServeDir` from `crates/agent-gateway/assets/`. In Docker, baked into the image.

---

## 6. File Structure

```
crates/agent-gateway/
├── Cargo.toml                       # + askama, askama_axum, axum-extra (cookie+multipart)
├── assets/                          # served at /assets/*
│   ├── style.css                    # ~600 lines, design system + components
│   ├── app.js                       # ~250 lines: streaming, composer state, theme toggle
│   ├── icons.svg                    # SVG sprite (one <symbol> per icon)
│   ├── sigil.svg                    # brand mark
│   └── fonts/                       # optional self-hosted (CDN by default)
├── templates/                       # askama default location
│   ├── login.html
│   ├── app.html                     # full shell (sidebar + main + composer)
│   ├── partials/
│   │   ├── sidebar.html
│   │   ├── greeting.html
│   │   ├── composer.html
│   │   ├── quick_chips.html
│   │   ├── message_user.html
│   │   ├── message_ai.html
│   │   ├── tool_card.html
│   │   ├── attachment.html
│   │   └── capability_card.html
│   └── shared/
│       ├── head.html                # meta, fonts, css link, theme bootstrap
│       └── flash.html               # error/success toast
└── src/
    ├── main.rs                      # mounts ui_router() + ServeDir("/assets")
    └── ui/
        ├── mod.rs                   # pub use routes::ui_router; pub use session::*;
        ├── routes.rs                # ui_router() — all UI routes in one place
        ├── session.rs               # cookie sign/verify, SessionUser extractor
        ├── view.rs                  # Askama Template structs (one per page)
        └── handlers/
            ├── mod.rs
            ├── auth.rs              # GET/POST /login, GET /logout
            ├── app.rs               # GET / → app shell, GET /app/recents (HTMX)
            ├── chat.rs              # POST /ui/stream (SSE), POST /ui/chat (partial), GET /ui/threads/:id (load history)
            └── upload.rs            # POST /ui/upload (multipart → MinIO)
```

---

## 7. Auth & Session (Mock)

- **Cookie**: `conusai_session = base64url(payload).hmac256(UI_SESSION_KEY)`
  - payload: `{ "name": "John Smith", "plan": "enterprise", "exp": 1714435200 }`
  - 24h expiry, `HttpOnly`, `SameSite=Lax`, `Secure` when `https`
  - HMAC key from `UI_SESSION_KEY` env var; auto-generated to `.conusai-session-key` file on first start (gitignored)
- **`/login` POST** form: `name` (required, 1–60 chars), `plan` (radio: free/pro/enterprise) → set cookie → 302 `/`
- **`/logout` GET** → clear cookie → 302 `/login`
- **`SessionUser` extractor** → axum extractor that validates the cookie; returns `Redirect::to("/login")` on failure. Used by every UI route except `/login` and `/assets/*`.
- **Bridge to gateway tenancy**: `SessionUser → TenantContext` mapping in `session.rs`:
  - `tenant_id = format!("ui-{}", slug(name))`
  - `plan = session.plan` (mapped to `PlanTier`)
  - This means the UI can call agent-core **directly** (in-process) with a constructed `TenantContext` — no HTTP self-call, no embedded API key in HTML.

---

## 8. Streaming Architecture

**No HTTP self-call.** The UI streams by invoking agent-core directly with the session-derived `TenantContext`. This requires extracting the streaming loop from `routes/agent.rs` into a reusable function:

```rust
// crates/agent-gateway/src/routes/agent.rs (refactor)
pub async fn stream_agent_to_channel(
    state: Arc<AppState>,
    tenant: TenantContext,
    req: AgentRequest,
    tx: tokio::sync::mpsc::Sender<sse::Event>,
) -> anyhow::Result<()> { /* existing streaming loop, but writes to tx */ }
```

**`POST /ui/stream`** handler:
1. Validate `SessionUser`.
2. Parse JSON body: `{ thread_id?, prompt, model?, attachments?: [url] }`.
3. If `thread_id` missing, create one via `state.thread_store`.
4. Append user message to thread.
5. Open SSE response (`text/event-stream`), spawn task that calls `stream_agent_to_channel`.
6. Channel events:
   - `event: token`  · `data: {"text": "..."}`
   - `event: tool_start` · `data: {"id": "...", "name": "...", "capability": "..."}`
   - `event: tool_result` · `data: {"id": "...", "result": {...}, "ms": 23}`
   - `event: done` · `data: {"thread_id": "...", "usage": {...}}`
   - `event: error` · `data: {"message": "..."}`

**Frontend `app.js` (sketch)**:
```js
async function send(prompt, files = []) {
  const res = await fetch('/ui/stream', {
    method: 'POST',
    headers: {'content-type': 'application/json'},
    body: JSON.stringify({ thread_id: state.threadId, prompt, attachments: files }),
  });
  const reader = res.body.getReader();
  const dec = new TextDecoder();
  let buf = '';
  while (true) {
    const {done, value} = await reader.read();
    if (done) break;
    buf += dec.decode(value, {stream: true});
    let i;
    while ((i = buf.indexOf('\n\n')) >= 0) {
      handleEvent(parseSSE(buf.slice(0, i)));
      buf = buf.slice(i + 2);
    }
  }
}
```

`handleEvent` dispatches:
- `token` → append text to current AI message bubble (one DOM `Text` node, replace contents to avoid layout thrash)
- `tool_start` → insert `<tool-card>` skeleton with status dot = `running`
- `tool_result` → fill the matching `<tool-card>` body, swap dot to `success`/`error`
- `done` → finalize bubble, persist `state.threadId`

---

## 9. Capability Rendering (registry-driven)

The sidebar's **Capabilities** section is rendered server-side from `state.registry`:

```rust
// view.rs
#[derive(Template)]
#[template(path = "app.html")]
struct AppView {
    user: SessionUser,
    recents: Vec<RecentThread>,    // from thread_store.list(tenant, 20)
    capabilities: Vec<CapView>,     // from registry
    greeting: String,               // time-of-day + name
    sigil_svg: &'static str,
}
struct CapView { name: String, kind: String, tool_count: usize }
```

Adding a new capability (Docker, WASM, MCP, pipeline) requires **zero frontend code** — the registry change appears automatically. Clicking a capability inserts `@capability_name` into the composer (Alpine.js handler) so users can target tools explicitly.

**Empty registry state**: shows a single "No capabilities loaded — see `capabilities/`" hint with mono label.

---

## 10. File Upload

- **`POST /ui/upload`** (multipart, `max-size: 25MB`) → reuses existing `/v1/files` MinIO logic via direct call to `files::upload_inner(state, tenant, parts)`.
- Returns `{ id, url, mime, bytes, name }`.
- Frontend pushes to `state.attachments[]`, renders `<attachment-chip>` with thumbnail (image/pdf glyph/audio glyph) inside composer.
- On submit, attachments are passed in the `/ui/stream` body. The agent prompt is augmented server-side: `\n\n[Attached: {name} ({url})]` until first-class multimodal message support lands.
- **`invoice.png` heuristic**: when an upload's filename matches `/invoice/i` or detected MIME is `image/*` with OCR-likely content, the composer pre-fills `Process this invoice and extract structured data.` (operator can edit before send).

---

## 11. Phase-by-Phase Implementation

Each phase ends with a **browser verification** step using Chrome (manual or via Chrome MCP `navigate` + `read_page` + `screenshot`). Phases are independently mergeable.

### Phase 0 — Foundation (≈1h)

1. Add deps to `crates/agent-gateway/Cargo.toml`:
   ```toml
   askama       = { workspace = true }
   askama_axum  = "0.4"
   axum-extra   = { version = "0.10", features = ["cookie", "multipart"] }
   ```
2. Create `assets/`, `templates/`, `src/ui/` skeleton (empty files per § 6).
3. Mount in `main.rs`:
   ```rust
   let app = Router::new()
       .merge(routes::public_router())
       .merge(routes::protected_router().layer(...))
       .merge(ui::ui_router())
       .nest_service("/assets", ServeDir::new("crates/agent-gateway/assets"))
       .layer(...);
   ```
4. Write `style.css` skeleton with all CSS variables (§ 3.1–3.4) and `@font-face` + `<link>` to Fraunces, Switzer, JetBrains Mono.
5. Verify: `cargo run -p agent-gateway`, visit `http://localhost:8080/assets/style.css` → 200, fonts load (DevTools → Network → Fonts).

### Phase 1 — Auth + Layout Shell (≈2h)

1. Implement `session.rs` (HMAC sign/verify, `SessionUser` extractor).
2. Implement `handlers/auth.rs` (`/login` GET+POST, `/logout`).
3. Build `templates/login.html`:
   - **Layout**: 2-column 50/50. Left = full-bleed ember gradient (`linear-gradient(135deg, #D9531E, #8B2E0E)`) with brand sigil top-left and tagline "ConusAI · agent workshop" set in Fraunces 32px. Right = cream paper, centered form: `Name` field (Fraunces 24px input, no border, ember underline on focus), plan radio chips, submit button "Enter the workshop" (mono uppercase label).
4. Build `templates/app.html` and partials:
   - **Sidebar** (260px, `--paper-2` bg, 1px ember left rule):
     - Brand monogram top (custom 28×28 sigil + "ConusAI" in Fraunces 16px, letter-spacing -0.01em)
     - Primary nav: New chat, Search (mono labels, 13px, with sprite icons)
     - Secondary nav: Chats, Projects, Code, Customize, Design
     - "RECENTS" label (mono uppercase 11px, tracking 0.14em, color `--ink-3`) + list (truncate at 32 chars, ember left-edge flash on hover via `::before { width: 0 → 3px }`)
     - "CAPABILITIES" label + capability cards (server-rendered from registry, empty state placeholder)
     - User chip pinned to bottom: avatar circle (initials in Fraunces), name, plan badge
   - **Main area**:
     - Top right: theme toggle (sun/moon glyph), help (?) — 18×18 icons, no labels
     - Centered greeting (Fraunces 48px, `opsz: 96`): `{sigil} Afternoon, John Smith`
     - Composer (max-width 720px, 1px `--rule` border, no box-shadow, inner padding 16px, focus ring = 1px ember)
     - Quick chips row (mono labels, 11px, ember underline grow on hover)
5. **Page-load orchestration** (§ 3.4 step 1–5) wired in `style.css` via `@keyframes` + `animation-delay`.
6. **Verify in Chrome**:
   - Visit `http://localhost:8080` → 302 `/login`.
   - Submit "John Smith" + Enterprise → 302 `/`.
   - Greeting reads correctly; sigil visible; sidebar items load with stagger; theme toggle switches Paper ↔ Forge and persists in `localStorage`.
   - Lighthouse a11y ≥ 95.

### Phase 2 — Chat + Streaming (≈3h)

1. Refactor `routes/agent.rs` to extract `stream_agent_to_channel(state, tenant, req, tx)`.
2. Implement `handlers/chat.rs::ui_stream` per § 8.
3. Implement `templates/partials/message_user.html`, `message_ai.html`.
4. Write `app.js`:
   - Composer submit handler: optimistic user-bubble append, POST `/ui/stream`, stream reader loop, AI bubble incremental fill.
   - Cursor pulse element appended to currently-streaming bubble; removed on `done`.
   - Auto-scroll only when user is within 80px of bottom (avoid yanking the viewport when reading history).
5. Implement `GET /ui/threads/:id` (HTMX endpoint) → renders message history as partials when user clicks a recent.
6. **Verify in Chrome**:
   - Type "Write a haiku about copper." → user bubble appears instantly, AI bubble streams token-by-token, cursor pulses, done event finalizes.
   - Click a "Recents" item → message history loads; composer remains empty.
   - Quick chip "Code" → composer pre-fills "Help me write code that …" with focus.
   - Open in narrow window (375px) → sidebar collapses to a left drawer (Alpine `x-show`), composer remains usable.

### Phase 3 — Tools + Capability Cards (≈2h)

1. Implement `templates/partials/tool_card.html`:
   - Header: `◆ {name}` (mono 12px) · `{capability}` (`--ink-3`) · `{ms}ms` (right-aligned tabular).
   - Status dot: 6×6, color = `running`(steel pulse) / `success`(moss) / `error`(rust).
   - Body: collapsible (`<details>` element), JSON pretty-printed in mono with subtle key/value distinction (`--ink-2` keys, `--ink` values).
2. `app.js`: insert tool card on `tool_start`, fill on `tool_result`. Cards render **inside** the AI bubble flow, between text segments.
3. Sidebar capability section: server-rendered from registry; clicking inserts `@cap_name ` into composer (Alpine handler).
4. **Verify in Chrome**:
   - "Read the README.md" → `read_file` tool card appears (running → success), result JSON visible on expand.
   - "Run cargo check" → `run_cargo` card with stdout snippet, error rendering if it fails.
   - Sidebar shows capability cards from `state.registry`; click `@native-tools` → mention inserted, focus moves to composer end.

### Phase 4 — File Upload + Attachments (≈2h)

1. Implement `handlers/upload.rs` (multipart → existing `files::upload_inner`).
2. Composer paperclip button: `<input type="file" hidden multiple>` triggered via Alpine.
3. Attachments preview row inside composer: thumbnail (image preview via `URL.createObjectURL`, glyph for non-image), name, size, ✕ remove.
4. Drag-and-drop on composer surface: dashed ember border on `dragover`, drop handler reuses upload flow.
5. `invoice.png` heuristic: matches → composer pre-fills suggested prompt.
6. **Verify in Chrome**:
   - Upload `docs/ui.png` → thumbnail appears in composer; submit; AI bubble references the attached URL.
   - Upload an `invoice.png` sample → composer pre-fills "Process this invoice…"; submit triggers OCR tool card.
   - Drag a PDF onto composer → drop animation plays, attachment chip appears.

### Phase 5 — Polish (≈2h)

1. **Keyboard shortcuts** (registered in `app.js`):
   - `⌘K` → focus search, list threads (HTMX-driven palette overlay)
   - `⌘N` → new chat (clears state, focuses composer)
   - `⌘/` → toggle theme
   - `⌘↵` in composer → send (in addition to plain Enter; Shift+Enter = newline)
   - `Esc` → close palettes / detach focused attachment
2. **Toast / flash** for errors (`partials/flash.html`), driven by `event: error` SSE messages.
3. **Empty states**: greeting shows hints when no recents; capability section shows discovery hint when registry is empty.
4. **Theme persistence**: `localStorage.theme = paper|forge`, applied pre-paint via inline script in `<head>` to avoid FOUC.
5. **Reduced motion**: `prefers-reduced-motion` media query disables all transforms, clamps durations.
6. **Update infra**:
   - `start.sh` → opens browser to `http://localhost:8080` after gateway boots.
   - `Dockerfile` → `COPY crates/agent-gateway/assets /app/assets` and `COPY crates/agent-gateway/templates /app/templates`.
   - `docker-compose*.yml` → expose 8080 (already done) + add `UI_SESSION_KEY` env or auto-mount volume for the key file.
   - `.env.example` → document `UI_SESSION_KEY`.
7. **Verify in Chrome**: full Phase 6 checklist.

### Phase 6 — Verification Checklist

Run after `cargo run -p agent-gateway` (or via `start.sh`).

**Functional**
- [ ] `/` redirects to `/login` when no cookie
- [ ] Login as "John Smith" / Enterprise → 302 `/`
- [ ] Greeting "Afternoon, John Smith" with sigil renders crisp at 1x and 2x DPI
- [ ] Sidebar "Recents" lists 20 newest threads (live from Qdrant)
- [ ] "Capabilities" section lists registry entries (count matches `/v1/capabilities`)
- [ ] New chat → streaming response, tokens fade in, cursor pulses, done event finalizes
- [ ] `read_file`, `write_file`, `run_cargo` tool cards appear with proper status transitions
- [ ] Upload PNG → thumbnail; upload PDF → glyph; drag-drop works
- [ ] `invoice.png` upload → composer pre-fill + OCR tool fires
- [ ] `⌘K`, `⌘N`, `⌘/`, `⌘↵`, `Esc` shortcuts all work
- [ ] Theme toggle Paper ↔ Forge; persists across reload; no FOUC
- [ ] Reload preserves session; `/logout` clears it

**Visual**
- [ ] Fonts loaded (Fraunces, Switzer, JetBrains Mono — DevTools → Network → Fonts shows 3 hits with `font/woff2`)
- [ ] Greeting set in Fraunces display optical size (wedge serifs visible)
- [ ] No purple gradients, no Inter, no rounded-everything
- [ ] Hairline rules at 1px (visible at 1x DPI without antialiasing artifacts)
- [ ] Ember accent appears only on: focus rings, sigil pulse, capability glyphs, tool status dots, hover left-edge, login button
- [ ] Page-load orchestration plays in order; reduced-motion respected

**Performance & a11y**
- [ ] Lighthouse: Performance ≥ 90, Accessibility ≥ 95, Best Practices ≥ 95
- [ ] Initial HTML payload < 40KB (gzipped); CSS < 20KB; JS < 12KB (excluding HTMX/Alpine CDN)
- [ ] LCP < 1.5s on cable; TTI < 2s
- [ ] All interactive elements keyboard-reachable; focus rings visible
- [ ] `role="log"` on message stream; `aria-live="polite"` on streaming bubble; `role="status"` on tool cards
- [ ] Color contrast ≥ 7:1 for body, ≥ 4.5:1 for meta (verified with axe DevTools)

---

## 12. Performance Budgets

| Metric | Budget | Notes |
|---|---|---|
| Initial HTML | 40 KB gzipped | full app shell (sidebar + greeting + composer) |
| CSS | 20 KB gzipped | hand-crafted, single file |
| JS (own) | 12 KB gzipped | streaming + composer state |
| JS (CDN: HTMX 2 + Alpine 3) | ~30 KB combined | acceptable; cached cross-site |
| Fonts | 3 woff2 files, ~140 KB total | display-swap, subset to Latin |
| LCP | < 1.5s | Greeting is LCP element |
| INP | < 200ms | streaming append must not block input |

---

## 13. Accessibility Spec

- Sidebar nav: `<nav aria-label="Primary">` + `<nav aria-label="Recents">` + `<nav aria-label="Capabilities">`.
- Composer: `<form>` with `<label class="sr-only" for="prompt">Message</label>`.
- Streaming bubble: container has `aria-live="polite" aria-atomic="false"`; finalized bubbles drop the live attr.
- Tool cards: `<details><summary role="button" aria-expanded="...">`; status dot has `aria-label="running|complete|error"`.
- Modal palette (`⌘K`): focus-trapped, `Esc` closes, returns focus to opener.
- All icons inside buttons paired with `aria-label`.
- Theme toggle: `<button aria-pressed="true|false">`.

---

## 14. Future Extensions (post-MVP, no UI rework needed)

- **Pipelines panel**: a third workspace beside chat (split view) reading `agent-core::pipelines` registry — same `CapabilityCard` component renders pipeline steps.
- **Eval runs**: live tail of an eval suite with the same `ToolCard` component for each step.
- **MCP debug console**: surface raw JSON-RPC traffic in a bottom drawer (mono, dimmed).
- **WASM tool sandbox console**: stdout/stderr stream into a tool card variant.
- **Real auth**: swap `session.rs` HMAC cookie for OIDC; templates and routes unchanged.

---

## 15. Acceptance Criteria

- The interface honors `docs/ui.png` structurally **and** asserts a distinct ConusAI identity (Foundry aesthetic, custom sigil, editorial type).
- Every backend feature in `docs/arch.md` (chat, threads, tools, files, capabilities, MCP) is reachable from the UI without bespoke clients.
- Capability registry drives the sidebar — adding a Docker/WASM/MCP capability requires zero UI changes.
- No HTTP self-call: UI streams via direct in-process invocation of agent-core with a session-derived `TenantContext`.
- All Phase 6 verification items pass.
- Code follows SRP, fully typed via Askama; the design system fits in one CSS file plus one JS file.
