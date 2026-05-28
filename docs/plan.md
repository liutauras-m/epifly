# Plan v4.1 — Workspace as Spatial Memory (threads, files & context in one graph)

**A workspace is not a folder system. It is a spatial memory database where conversations, files,
decisions, and context share one durable structure.**

This supersedes the backend agent-gateway/workspaces roadmap (Phases 0–5, complete) and revises
Plan v4 after a UX review. The correction that drove this revision: v4 thought like a *file
manager*; humans retrieve knowledge through **five overlapping paths** — location, recency,
meaning, task-state, and conversation trail — and a pure hierarchy serves only the first. The tree
is the durable *home*; metadata, smart views, search, and a memory layer are the other paths.

Same execution discipline: step-numbered, one concern per phase, contract-before-code, test-first
where verifiable.

---

## The product must always answer five user questions

Every screen is judged against these. If the UI answers them, the workspace feels seamless; if not,
it's "Finder with a chatbot taped on."

1. **Where am I?** (breadcrumb + active place)
2. **What am I working on?** (thread/object title + state)
3. **What does the assistant know here?** (visible ambient context)
4. **Where will this be saved?** (destination / Unsorted, never a mystery)
5. **How do I find it later?** (Recents, Tree, Smart Views, Search — four retrieval paths)

## Mental model: Place · Object · Conversation · Memory

A conceptual lens, not four new tables. Object and Conversation are both `WorkspaceNode`s
distinguished by `semantic_kind`; Memory is the derived layer.

- **Place** — a folder/workspace/context. *"Where does this belong?"* (`Clients / Kebab Inn`)
- **Object** — a durable thing: file, note, PDF, generated doc, task. *"What is this?"*
- **Conversation** — an *interactive* object: transcript + summary + decisions + linked files +
  extracted tasks + state. *"What did we work through?"*
- **Memory** — derived: summaries, embeddings, entities, relations, unresolved questions,
  preferences. *"What should the assistant know next time?"*

The product breakthrough is making Object + Conversation + Memory **one object graph** with a
**spatial interface over structured memory** — not folders with chat bubbles.

## Design principles (rationale, grounded in known HCI)

- **Piles *and* files** (Malone, 1983): keep **Recents** *and* the **Tree**. Never force one.
- **Recognition over recall / spatial memory** (Nielsen; method-of-loci): the **tree never
  auto-sorts**; only Recents re-sorts by recency.
- **Multiple retrieval paths** (NN/g IA): location → tree; time → Recents; meaning → search;
  state → Smart Views. One nav pattern can't satisfy all four.
- **Direct manipulation + object permanence**: drag-to-folder *is* the move; delete shows
  `[Restore]`, never a silent vanish.
- **Progressive disclosure**: the tree shows *identity*, not content; the body/summary appears on
  demand.
- **Suggest, never silently act**: the system may *propose* a folder; the user confirms. Silent
  auto-organization kills trust.
- **Shallow by default**: cap practical depth at ~4 levels via defaults and suggestions — guidance,
  not an enforced wall (power users may go deeper).
- **Avoid mode-confusion**: branch UI on `semantic_kind`, never on storage `kind`/mime.

## UI vocabulary (hide the engineering terms)

| Internal (code) | User-facing (UI) |
|---|---|
| Thread / thread projection | **Conversation** |
| WorkspaceNode | **Item** / **Workspace** |
| Projection / semantic_kind / source_id | *(never shown)* |
| Document node | **Document** |
| Ambient context / workspaceNodeId | **Context** ("Using context from …") |
| `hidden_at` set | **Paused** |
| Restore (clear `hidden_at`) | **Restore** |
| move | **Move to…** |
| getContent peek | **View as document** |

---

## Current-state audit — what already works (do NOT re-implement)

Verified 2026-05-28:

- **Backend** `common/src/memory/workspace.rs`: `WorkspaceNodeKind { Folder, File, Thread }`;
  `WorkspaceNode` carries `semantic_kind`, `source_type` (`"thread_projection"`), `source_id`
  (originating `thread_id`), `hidden_at` (pause), `tags`, `metadata: serde_json::Value`,
  `virtual_path`, `last_modified`. **`metadata` is the no-migration home for new work-unit fields.**
- **Thread projection** (`agent-core` + `jobs`): durable, coalesced; factory + contract tests done.
- **SDK** (`packages/sdk/src/workspaces.ts`): `tree`, `get`, `create`, `search`, `getContent`,
  `move({new_parent_id,new_parent_path})`, `delete`, `putTags`, `filterNodes({tag,kind,since,q,limit})`.
- **TS type** (`packages/types/src/domain.ts`): `WorkspaceNode.semantic_kind`, `source_id`,
  `source_type`, `tags`.
- **UI tree row** (`workspace-node-row.svelte`): already renders `kind === "thread"` with a chat
  glyph; folder expand/collapse, draft create, selection, keyboard, ARIA `treeitem` done.
- **Stores**: `createWorkspacesStore`, `createThreadsStore`, `createAppShellState` (merge point at
  `app-shell-state.svelte.ts:47`).

## Known gaps this plan closes

| # | Gap | Where | Phase |
|---|-----|-------|-------|
| G1 | Adapter branches on `node.kind` not `semantic_kind` → threads map to `"document"`; **breaks invariant #12** | `workspace-adapters.ts:35` | 0 |
| G2 | `SidebarWorkspaceNode` lacks `threadId`/`virtualPath`/work-unit fields | `workspace-adapters.ts:9` | 0 |
| G3 | Unverified whether `/v1/workspaces/tree` returns Thread nodes | backend route | 0 |
| G4 | Clicking a thread node selects, doesn't open the conversation | row → `app-shell-state` | 1 |
| G5 | `sortRecentFirst` re-sorts tree every load → breaks spatial stability | `workspaces.store.svelte.ts:127` | 1 |
| G11 | No breadcrumb / location awareness in chat header | chat pages | 1 |
| G12 | No "Unsorted" home — new threads can feel orphaned | sidebar + projection default | 2,3 |
| G13 | No Smart Views (retrieval by state/time) | sidebar IA | 2 |
| G6 | No drag-and-drop → `move()` never called from UI | `workspace-node-row.svelte` | 3 |
| G14 | No suggested filing (and risk of silent auto-move) | features + backend | 3 |
| G17 | No command palette (Cmd+K) | new UI | 3 |
| G7 | No "view as document" peek | new UI | 4 |
| G8 | No restore endpoint to un-pause a thread | backend + SDK | 5 |
| G9 | Thread folder location not fed to chat as context | chat send | 6 |
| G15 | No visible active-context indicator | chat header | 6 |
| G10 | No optimistic thread node / syncing state on first message | chat store + tree | 7 |
| G16 | No relationship/memory layer for search, related-items, suggestions | backend | 8 |
| G18 | `WorkspaceNode` has no `status`/`summary`/relations to behave like a work unit | metadata first | 0,8 |

---

## Sidebar information architecture (target)

Replaces today's two sections (Files + Chat history) with **four lanes**, weighted by how users
actually retrieve:

```
Recents            ← time   ("what I touched")        dynamic, ≤8
Workspace (Tree)   ← place  ("where I left it")        stable, user-owned
Smart Views        ← state  (Unsorted, Paused, …)      filters, not folders
Search / Cmd+K     ← meaning + action                  command-first
```

Each lane is visually distinct (not equal weight). Tree rows are dense: `icon + title + optional
tiny metadata` — metadata only on hover/selection/peek.

## Non-negotiable invariants

1. **Branch on `semantic_kind`, never on `kind`/mime.** Adapter is the single translation point.
2. **Delete of a Conversation = pause, not destroy.** Always offer `[Restore]`. Never hard-delete a
   thread node; never silently resurrect one.
3. **Tree order is user-owned.** Never reorder the tree under the user. Recency lives in Recents.
4. **One `node_id`, one identity.** Recents, Tree, and Smart Views render the *same* node; selecting
   in one reflects in all.
5. **Suggest, never silently act.** The system may propose a destination; the user confirms.
   Placement and order stay user-owned.
6. **Nothing is orphaned.** Every conversation has a visible home (real folder or the **Unsorted**
   view) from the moment it exists.
7. **Context is visible.** When the assistant uses ambient context, the UI says which place.
8. **Optimistic, never blocking.** Projection is async; show a "syncing" affordance, never block
   chat or the tree.
9. **`packages/ui` stays SDK-free and features-free.** Props in, callbacks out.
10. **Every pointer action has a keyboard path.** Move/rename/delete reachable without a mouse.
11. **Shallow by default** (≈4 levels) via defaults + suggestions — guidance, not a hard cap.
12. **User-facing vocabulary** per the table above; engineering terms never surface.

---

## Phase 0 — Data plumbing correctness + work-unit foundation

**Goal:** thread-projection nodes flow through the adapter as `kind:"thread"` carrying `threadId`,
`virtualPath`, and optional work-unit fields. This single fix makes threads appear in the tree.

### Step 0.1 — Adapter honors `semantic_kind` (test-first)
- **Contract:** `toSidebarWorkspaceNode` maps `WorkspaceNode.semantic_kind` →
  `"folder"|"thread"|"document"` (Thread→`"thread"`). It no longer reads `node.kind`.
  `SidebarWorkspaceNode` gains `threadId?: string|null` (from `source_id` when thread) and
  `virtualPath?: string`.
- **Files:** `packages/features/src/workspaces/workspace-adapters.ts`; mirror optional fields in
  `packages/ui/.../workspace-tree.svelte`.
- **Test (first):** `workspace-adapters.test.ts` — a `thread_projection` node with
  `semantic_kind:"thread"`, `source_id:"t_123"`, `virtual_path:"Clients/Acme/Kickoff"` →
  `{ kind:"thread", threadId:"t_123", virtualPath:"Clients/Acme/Kickoff" }`. Folder→folder,
  file→document.

### Step 0.2 — Confirm the tree endpoint returns Thread nodes
- **Contract:** `GET /v1/workspaces/tree` includes `semantic_kind:"thread"` nodes with
  `hidden_at IS NULL`; excludes paused ones. If currently filtered, include them (threads are part
  of the tree by definition).
- **Files:** agent-gateway tree route + handler.
- **Test:** testcontainer — project a thread, GET tree, assert presence + correct `parent_id`;
  set `hidden_at`, assert exclusion.

### Step 0.3 — Drop the lossy `"conversation"` fallback
- **Contract:** remove `case "conversation": return "document"`; storage `kind` is no longer
  consulted for semantics. **Covered by 0.1.**

### Step 0.4 — Work-unit fields foundation (no migration)
- **Contract:** `SidebarWorkspaceNode` carries optional pass-throughs sourced from `metadata` +
  existing columns: `status?: "active"|"paused"|"done"|"archived"`, `summary?: string`,
  `lastActivityAt?: string` (from `last_modified`), `tags: string[]`, `relatedNodeIds?: string[]`.
  No UI behavior yet — these enable Smart Views (Phase 2) and Memory (Phase 8). New backend writes
  land in `WorkspaceNode.metadata`; promote to typed fields only once a field proves stable.
- **Files:** adapter + UI node type; document the `metadata` sub-schema in code comments.
- **Test:** adapter passes through `status`/`summary`/`tags` from `metadata` when present; absent →
  `undefined`/`[]`.

**Gate:** `pnpm --filter @epifly/features test` + `svelte-check` (features, ui). Manually load the
tree; thread nodes appear with the chat glyph.

---

## Phase 1 — Spatial identity (where am I, what's active)

**Goal:** open a conversation from the tree; Recents fast-lane; stable order; breadcrumb + active
context in the chat header.

### Step 1.1 — Open-as-chat on thread nodes
- **Contract:** rows gain `onOpenThread?(threadId)`. A `kind:"thread"` row invokes it on primary
  click instead of `onSelect`. `app-shell-state` wires `onOpenThread = id => navigate('/chat/'+id)`.
- **Files:** `workspace-node-row.svelte`, `workspace-tree.svelte`, `app-navigation-sidebar.svelte`,
  `app-shell-state.svelte.ts`, both `(app)/+layout.svelte`.
- **Test:** clicking a thread row calls `onOpenThread(threadId)`; a document row still `onSelect`.

### Step 1.2 — Recents fast-lane (piles + files)
- **Contract:** "Recents" lane of ≤8 most-recent conversations, alongside the tree, reusing the same
  `threadId`/route. The full set lives in the tree.
- **Files:** `app-navigation-sidebar.svelte`, `app-shell-state`.
- **Test:** Recents ≤8; a Recents item and the same thread's tree node route identically.

### Step 1.3 — Spatial stability (tree never auto-sorts)
- **Contract:** remove `sortRecentFirst` from `loadTree`/`loadChildren`; apply recency **only** in
  Recents. Realtime refresh must not reorder siblings.
- **Files:** `workspaces.store.svelte.ts`.
- **Test:** load → realtime refresh → sibling order byte-identical.

### Step 1.4 — Breadcrumb in chat header (NEW)
- **Contract:** opening a thread shows its workspace breadcrumb (e.g. `Clients / Kebab Inn /
  Ordering`) derived from the node's `virtualPath`. Clicking a crumb opens that folder in the tree.
- **Files:** `(app)/chat/[threadId]/+page.svelte` + new `chat-breadcrumb.svelte` (presentational),
  features helper to resolve folder from the thread's node.
- **Test:** a foldered thread renders the correct crumb path; crumb click selects the folder node.

### Step 1.5 — Active context indicator (NEW)
- **Contract:** chat header shows a subtle "Context: <place>" chip reflecting the ambient context
  that *will* be used (wired fully in Phase 6; here it shows the resolved place or "None").
- **Files:** chat header component.
- **Test:** indicator shows the thread's place; "None" for an unfiled thread.

**Gate:** features tests + `svelte-check`; manual: open from tree and Recents (identical
destination); breadcrumb correct; tree order stable across realtime refresh.

---

## Phase 2 — Smart Views (retrieval by state & time)

**Goal:** add the Smart Views lane — *filters, not folders* — so users retrieve by state/time, not
only by place. Establishes the framework + the views whose dependencies already exist.

### Step 2.1 — Smart Views lane framework
- **Contract:** new sidebar lane rendering a list of named views; selecting one shows a flat,
  filtered result list (same row component, same `node_id` identity). Backed by
  `sdk.workspaces.filterNodes(...)`.
- **Files:** `app-navigation-sidebar.svelte` (4-lane IA), new `smart-views.svelte` (presentational),
  features `smartView(kind)` action.
- **Test:** selecting a view calls `filterNodes` with the right params and renders results.

### Step 2.2 — "Unsorted" view
- **Contract:** lists conversations not yet filed by the user (still at the projection default
  location / no user-assigned parent). Gives every thread a visible home (invariant #6).
- **Files:** features `smartView("unsorted")`; backend filter support for "unfiled" if not derivable
  client-side (verify `filterNodes` can express it; else add a `unfiled=true` param).
- **Test:** a freshly projected, unmoved thread appears in Unsorted; moving it removes it.

### Step 2.3 — "Recently updated" view
- **Contract:** flat list sorted by `last_modified` desc across kinds. (This is recency *as a view*,
  distinct from the stable tree.)
- **Test:** ordering matches `last_modified` desc.

> **Deferred views (dependencies elsewhere):** **Paused** → Phase 5 (needs restore + `hidden_at`
> filter); **Needs review** → Phase 8 (needs a *defined* trigger, e.g. extracted unresolved
> questions or explicit user flag — not shipped as a vague filter).

**Gate:** features tests + `svelte-check`; manual: Unsorted shows new threads; Recently-updated
orders correctly; lanes are visually distinct.

---

## Phase 3 — Safe organization (direct + assisted)

**Goal:** re-file by drag or command; rename; default new threads to Unsorted; *suggest* a home
without ever silently moving.

### Step 3.1 — Drag-and-drop move (+ accessible "Move to…")
- **Contract:** rows draggable; folders are drop targets (highlight on dragover). Drop emits
  `onMove(nodeId, newParentId, newParentPath)`; `workspacesStore.moveNode` applies optimistically,
  calls `sdk.workspaces.move(id,{new_parent_id,new_parent_path})`, reverts on error. Reject nesting
  a folder in itself and disallowed `is_protected_root` drops. **Also** expose a keyboard/touch
  "Move to…" menu (DnD alone is inaccessible).
- **Files:** `workspace-node-row.svelte` (DnD + menu), `workspaces.store.svelte.ts` (`moveNode`),
  sidebar + `app-shell-state` wiring.
- **Test:** `moveNode` relocates optimistically and calls `move` with target `id` + `virtualPath`;
  SDK error → revert.

### Step 3.2 — Rename in place
- **Contract:** double-click / F2 / context "Rename" → inline input (reuse draft UX). For a
  conversation this renames the node, not the transcript. Verify/add a rename route before assuming.
- **Test:** rename commits optimistically; reverts on error.

### Step 3.3 — Unsorted as default home (no orphan state)
- **Contract:** a new conversation has a visible home immediately — it appears in **Unsorted**
  (Phase 2) until filed. No upfront folder prompt. (Forcing classification before value exists is
  backwards.)
- **Files:** projection default location (confirm backend default `folder_path`), Unsorted view.
- **Test:** start a chat → it shows in Unsorted without any user filing.

### Step 3.4 — Suggested filing (suggest → confirm, never auto-move)
- **Contract:** once a conversation has enough content, surface a non-modal suggestion chip:
  `Suggested location: Product / OrdeON / Payments  [Move here] [Choose another] [Ignore]`.
  **Engine starts heuristic** — current route/open folder + linked files + title match — and may
  upgrade to embeddings later (gated). The system **never** moves without explicit confirmation.
- **Files:** features `suggestPlacement(threadId)` (heuristic v1), suggestion chip component, backend
  endpoint only when upgrading beyond heuristics.
- **Test:** suggestion renders a candidate; "Ignore" leaves location unchanged; "Move here" calls
  `move`; **no path performs a move without confirmation** (assert).

### Step 3.5 — Command palette (Cmd+K)
- **Contract:** global `Cmd/Ctrl+K` opens a command palette: *Move to…, Rename, New folder, Search
  workspace, Attach current folder as context, View as document, Pause conversation, Restore,
  New chat*. Commands route to the same store actions as their UI affordances.
- **Files:** new `command-palette.svelte` (presentational + keymap), features command registry,
  mount in both app layouts.
- **Test:** `Cmd+K` opens; "Move to…" runs the same `moveNode` path; Escape closes, restores focus.

**Gate:** features tests + `svelte-check`; manual: drag thread→folder persists; suggestion never
auto-moves; Cmd+K runs move/rename/search.

---

## Phase 4 — Dual representation (peek-as-doc)

**Goal:** the projected document is readable on demand; the tree shows identity only.

### Step 4.1 — "View as document" peek
- **Contract:** thread row secondary action (hover/overflow) "View as document" fetches
  `sdk.workspaces.getContent(nodeId)` and shows rendered Markdown read-only in a peek panel. Primary
  click still opens chat.
- **Files:** new `workspace-doc-peek.svelte`, features `peekWorkspaceDoc(nodeId)`, row wiring.
- **Test:** peek fetches content for the node id, renders read-only; close restores focus.

### Step 4.2 — Progressive-disclosure guardrail
- **Contract:** never inline the full body in the tree; only name + glyph + optional one-line
  preview. Body appears only in the peek.
- **Test:** tree row DOM contains no transcript body.

### Step 4.3 — Generated summary preview (NEW)
- **Contract:** when `summary` (Phase 0.4 / metadata) exists, show it as a single muted line on
  row hover/selection and atop the peek. One line max — no "Christmas-tree" rows.
- **Files:** row + peek; reads `node.summary`.
- **Test:** row with `summary` shows one preview line on hover; without it, none.

**Gate:** features/ui tests + `svelte-check`; manual peek shows transcript + summary.

---

## Phase 5 — Lifecycle (delete = pause → restore)

**Goal:** deleting a conversation pauses it (hides) and offers Restore. Invariants #2/#6.

### Step 5.1 — Backend restore endpoint
- **Contract:** `POST /v1/workspaces/{id}/restore` clears `hidden_at` for a Thread node (404/no-op
  otherwise). Confirm `DELETE` on a Thread sets `hidden_at` (fix if it hard-deletes). Add
  `EP.WORKSPACE_RESTORE` + SDK `workspaces.restore(id)`.
- **Files:** agent-gateway routes, `endpoints.ts`, SDK.
- **Test:** testcontainer — delete → `hidden_at` set + excluded from tree; restore → cleared +
  reappears; folder/file delete unchanged.

### Step 5.2 — Restore UX
- **Contract:** deleting a conversation shows "Conversation paused — [Restore]" (transient +
  reachable later). Hard delete never offered for conversations.
- **Files:** sidebar, features `deleteNode`/`restoreNode`, recently-paused state.
- **Test:** delete → leaves tree + Restore shown; Restore → returns at its folder.

### Step 5.3 — "Paused" smart view (NEW)
- **Contract:** populate the Phase 2 framework with a **Paused** view listing `hidden_at IS NOT
  NULL` threads, each with Restore.
- **Files:** features `smartView("paused")` (filter on hidden), sidebar.
- **Test:** paused threads appear only here; Restore removes them from the view.

**Gate:** integration + features tests; manual delete/restore round-trip; secret-leak grep on new
route; confirm no path hard-deletes a Thread node.

---

## Phase 6 — Ambient context (spatial → semantic, made visible)

**Goal:** a conversation's *location* feeds context, and the UI says so.

### Step 6.1 — Thread folder → chat `workspaceNodeId`
- **Contract:** continuing a thread in folder F includes F's `node_id` in `sdk.chat.stream`
  (`workspaceNodeId`). After a move (Phase 3) subsequent turns use the new folder.
- **Files:** `(app)/chat/[threadId]/+page.svelte`, `workspace-context.svelte.ts`, chat send args.
- **Test:** sending in a foldered thread passes that folder's node id.

### Step 6.2 — Visible context disclosure (NEW)
- **Contract:** the Phase 1.5 indicator becomes live: `Using context from: Product / OrdeON /
  Payments`, with a way to detach/override. Invisible context is powerful until it's wrong/creepy.
- **Files:** chat header chip (live), `attach current folder as context` command (Phase 3.5) wired.
- **Test:** indicator reflects the active `workspaceNodeId`; detaching clears it from the next send.

### Step 6.3 — Ambient retrieval bias (flagged, default off)
- **Contract:** retrieval biases toward siblings under the thread's `folder_path`; behind a config
  flag, default off until measured. Preserve routing audit fields.
- **Files:** agent-core retrieval/context builder.
- **Test:** flag on → sibling docs rank above unrelated for a foldered thread.

**Gate:** workspace + agent-core tests; manual: move a thread into a folder with a file, ask about
it, confirm the file is in context and the indicator names the place.

---

## Phase 7 — Optimistic + realtime polish

**Goal:** a new conversation appears instantly and shows projection progress.

### Step 7.1 — Optimistic thread node on first message
- **Contract:** on the first turn yielding a `thread_id`, insert an optimistic `kind:"thread"` node
  into **Unsorted**/root with a "syncing" flag, reconciled (same id) on the `workspace.*` realtime
  event. Never block chat.
- **Files:** chat store / app-shell glue, `workspaces.store` (`upsertOptimisticThreadNode`),
  realtime handler.
- **Test:** simulate first-message `thread_id` → node appears syncing → reconciles on invalidation.

### Step 7.2 — Syncing indicator + "still indexing" affordance
- **Contract:** mid-projection rows show a subtle indicator (dot/spinner, ≤8px, 120–240ms) with a
  "Still indexing…" tooltip; cleared on reconcile.
- **Files:** `workspace-node-row.svelte` (`syncing` prop), `motion.css` if needed.
- **Test:** `syncing` row renders indicator + tooltip; otherwise none.

**Gate:** features/ui tests + `svelte-check`; manual: start a brand-new chat, watch the node appear
in Unsorted and settle.

---

## Phase 8 — Memory layer (backend intelligence, no graph UI)

**Goal:** make the workspace a *memory database*: store relationships and status, power search /
related-items / suggestions. **No visual graph** (graph UIs become beautiful nonsense). Start with
edges we already know; defer NLP.

### Step 8.1 — Relationship fields from known signals (no NLP)
- **Contract:** in `WorkspaceNode.metadata`, maintain `relatedNodeIds`, `linkedFileIds` (chat
  attachments), `sourceThreadIds`, `derivedTaskIds` — all derivable from existing events (uploads,
  projection source, generated docs). No entity extraction yet.
- **Files:** projection + upload/generation paths write these; document the metadata sub-schema.
- **Test:** projecting a thread that referenced an uploaded file records the file in `linkedFileIds`.

### Step 8.2 — "Related items" surfacing
- **Contract:** the peek panel (Phase 4) shows a "Related" list from 8.1 (`relatedNodeIds` +
  `linkedFileIds`), each routing to its node. Read-only; no graph canvas.
- **Test:** peek lists related items; clicking routes to the right node.

### Step 8.3 — Status + "Needs review" (defined trigger)
- **Contract:** add `status` (`active|done|archived`; `paused` already = `hidden_at`) in metadata,
  settable via command/menu. **"Needs review"** smart view uses a *concrete* trigger — explicit user
  flag and/or extracted unresolved questions — never a vague heuristic.
- **Files:** features `setStatus`, Smart Views (`needs-review`), command palette entries.
- **Test:** flagging a conversation surfaces it in Needs review; clearing removes it.

### Step 8.4 — Entity extraction (DEFERRED, gated)
- **Contract:** `mentionedEntities` via NLP over transcripts/docs, feeding search ranking and
  suggestions (Phase 3.4 upgrade). Explicitly future; build only after 8.1–8.3 prove the surface.
- **Test:** TBD when scoped.

**Gate:** workspace + agent-core tests; manual: related items appear in peek; status changes drive
Smart Views.

---

## Per-phase gates (mandatory before next phase)

- `svelte-check` on touched packages (`@epifly/ui`, `native`, `web`) → 0 errors.
- `pnpm --filter @epifly/features test` for store/adapter changes.
- Backend-touching steps (0.2, 5.1, 6.3, 8.x): `cargo clippy --workspace --all-targets -- -D
  warnings` + `cargo test --workspace`; routes/storage use testcontainers.
- Phase boundaries only: `pnpm test:e2e:web`.
- 5–10 line eval per phase (closed items, deferred followups) in the PR description.
- **Acceptance lens:** every phase must still answer the five user questions for its surface.

## Execution checklist

- [x] 0.1 Adapter honors `semantic_kind` (+`threadId`,`virtualPath`) — test-first
- [x] 0.2 Tree endpoint returns Thread nodes (hidden filtered)
- [x] 0.3 Remove `"conversation"` fallback
- [x] 0.4 Work-unit fields foundation (metadata pass-through)
- [x] 1.1 Open-as-chat on thread nodes
- [x] 1.2 Recents fast-lane (≤8)
- [x] 1.3 Spatial stability (tree never auto-sorts)
- [x] 1.4 Breadcrumb in chat header (crumb-click wired 2026-05-28)
- [x] 1.5 Active context indicator (static)
- [x] 2.1 Smart Views lane framework
- [x] 2.2 Unsorted view
- [x] 2.3 Recently-updated view
- [x] 3.1 Drag-and-drop move (+ accessible "Move to…")
- [x] 3.2 Rename in place
- [x] 3.3 Unsorted as default home
- [x] 3.4 Suggested filing (suggest→confirm, heuristic v1) (2026-05-28)
- [x] 3.5 Command palette (Cmd+K)
- [x] 4.1 View-as-document peek
- [x] 4.2 Progressive-disclosure guardrail
- [x] 4.3 Generated summary preview
- [x] 5.1 Restore endpoint + SDK + delete=soft-delete
- [x] 5.2 Restore UX
- [x] 5.3 Paused smart view (?paused=true backend param, 2026-05-28)
- [x] 6.1 Thread folder → chat `workspaceNodeId`
- [x] 6.2 Visible context disclosure (live)
- [x] 6.3 Ambient retrieval bias (flagged, default off) — CONUS_WORKSPACE_SIBLING_BIAS env var, agent-core ContextBuilder, 4 tests, 2026-05-28
- [x] 7.1 Optimistic thread node
- [x] 7.2 Syncing indicator + "still indexing"
- [x] 8.1 Relationship fields from known signals — attachment_ids → ThreadProjectionInput → WorkspaceNode.metadata, 3 tests, 2026-05-28
- [x] 8.2 Related-items surfacing — workspace-peek.store resolves related_node_ids + linked_file_ids, PeekRelatedItem, navigateToRelated, 2026-05-28
- [x] 8.3 Status + Needs-review (defined trigger)
- [ ] 8.4 Entity extraction (deferred, gated) — EXPLICITLY OUT OF SCOPE

## Notes for the executing agent

- **Phase 0 is load-bearing.** Until the adapter reads `semantic_kind`, every later phase is
  invisible. Do it first, test it first.
- **Verify before assuming backend shape** (0.2, 2.2, 3.2, 5.1): read the agent-gateway routes and
  the projection's default `folder_path` before writing; don't assume an endpoint/param exists.
- **New persisted fields go in `metadata` first.** Promote to typed columns only when a field has
  earned it. No speculative migrations.
- **Suggest, never auto-move** (invariant #5) and **nothing orphaned** (invariant #6) are the two
  rules the UX review cared about most — assert them in tests, not just prose.
- **No graph UI.** The relationship layer is backend intelligence for search/related/suggestions.
- **DnD is never the only path.** Move/rename/delete/pause/restore all reachable via menu + Cmd+K.
- **One `node_id`, every lane.** Recents, Tree, Smart Views, and search results must select/route to
  the same node — never fork identity between a "conversation" and its "node".
- **Keep engineering terms out of the UI** (vocabulary table). Users should never read "projection".
- **Respect motion rules** (≤8px, 120–240ms, `prefers-reduced-motion` handled) for drag ghost,
  syncing dot, peek, palette.
- **Stop condition:** if a step needs work outside its phase's scope, write a short "unplanned
  scope" note before continuing.

---

## Phase V — Visual & device verification (iOS native + Web)

Run this **at every phase boundary and before any release.** Each user-facing surface from
Phases 1–7 must look and behave correctly on **both runtime apps** — `apps/web` and `apps/native`
(iPhone 16 Pro simulator) — in **light and dark**, at **mobile (≈390 px) and desktop (≥1280 px)**.
Backend-only steps (0.2, 6.3, 8.1) are proven by their own tests, not here. The acceptance lens
stays the **five user questions** — every screen must still answer them.

### V.0 — Launch harness

**Web**
```bash
pnpm --filter web run dev          # http://localhost:5173 (.claude/launch.json → "web")
```
Resize the viewport to verify all three breakpoints: 390 px (mobile), 768 px (md cutover), ≥1280 px.

**iOS native**
```bash
cd apps/native && pnpm tauri ios dev "iPhone 16 Pro"
xcrun simctl io booted screenshot /tmp/epifly.png    # capture current screen
```

### V.1 — iOS tap/inspect method (hard-won reference)

`osascript` System-Events `click` needs Accessibility permission this environment lacks. Drive taps
with CoreGraphics events and read state from simctl screenshots:

1. Activate the Simulator and **re-query window bounds each session** (the window moves):
   `osascript -e 'tell app "System Events" to tell process "Simulator" to get {position, size} of window 1'`.
2. simctl screenshots are 2× Retina of the logical window → `scale = win_w / shot_w` (≈0.33).
   Map: `mac_x = win_x + shot_x*scale`, `mac_y = win_y + shot_y*scale`. For a precise target,
   `screencapture -x -R 'win_x,win_y,win_w,win_h' /tmp/win.png` and read the pixel directly.
3. Tap via Python `ctypes` → `CGEventCreateMouseEvent` (mouseMoved → leftDown → leftUp) posted to
   `kCGHIDEventTap`; screenshot again to confirm.

### V.2 — Per-feature visual checklist (verify on BOTH platforms)

| Feature (phase) | What to see | iOS | Web |
|---|---|:--:|:--:|
| Threads in tree (0–1) | Conversations render inside folders with the chat-bubble glyph | ☐ | ☐ |
| Open-as-chat (1.1) | Tapping a conversation node opens that thread | ☐ | ☐ |
| Recents lane (1.2) | ≤8 recents; same item routes identically as its tree node | ☐ | ☐ |
| Stable order (1.3) | Tree does not reshuffle after sending a message / refresh | ☐ | ☐ |
| Breadcrumb (1.4) | Chat header shows `A / B / C`; crumb tap opens that folder | ☐ | ☐ |
| Context chip (1.5/6.2) | "Using context from …" reflects the place; detach works | ☐ | ☐ |
| Smart Views (2) | Lane lists Unsorted / Recently updated; selecting filters | ☐ | ☐ |
| Drag-to-folder (3.1) | Drag re-files; "Move to…" menu works without a mouse | ☐ | ☐ |
| Rename (3.2) | Inline rename commits / reverts on error | ☐ | ☐ |
| Suggested filing (3.4) | Suggestion chip appears; "Ignore" leaves it put; never auto-moves | ☐ | ☐ |
| Command palette (3.5) | Cmd/Ctrl+K opens; runs Move/Rename/Search; Esc restores focus | ☐ | ☐ |
| View-as-document (4.1) | Peek shows rendered Markdown read-only; tree shows no body | ☐ | ☐ |
| Summary line (4.3) | One muted preview line on hover/selection — never multi-line | ☐ | ☐ |
| Delete = pause (5) | Delete shows "Paused — Restore"; Paused view lists it; restore returns it | ☐ | ☐ |
| Optimistic node (7.1) | New chat → node appears immediately in Unsorted | ☐ | ☐ |
| Syncing state (7.2) | "Still indexing…" indicator shows then clears | ☐ | ☐ |

### V.3 — Cross-cutting visual checks

- **Safe area (iOS):** top toggles clear the Dynamic Island; composer clears the home indicator;
  nothing tucks under the floating toggles (the `--toggle-bar-height` fix). Portrait **and** landscape.
- **Responsive:** sidebar is a **Sheet** on mobile and a **persistent rail** at ≥768 px; no element
  overlap at the breakpoint.
- **Keyboard (iOS):** tapping the composer focuses it; the soft keyboard resizes content
  (`interactive-widget=resizes-content`) rather than covering the input.
- **Scroll:** a new message snaps to bottom; scrolling up during streaming is not yanked back.
- **Motion:** transitions ≤8 px / 120–240 ms; `prefers-reduced-motion` disables them.
- **Theme:** light/dark parity for tree, chat, peek, palette, Smart Views.
- **States:** empty (no conversations), loading (skeletons), and error states render correctly.
- **A11y:** every pointer action (move/rename/delete/pause/restore) reachable via menu + Cmd+K;
  focus rings visible; tree exposes `treeitem`/`aria-selected`.

### V.4 — Evidence & sign-off

- Capture before/after screenshots per feature (iOS via `simctl`, Web via browser) and attach to the
  phase PR.
- A phase is "visually verified" only when **every V.2 row is ticked on both platforms** and V.3
  passes. Record a one-line sign-off per platform in the PR (e.g. `iOS ✓ / Web ✓ — Phase 3`).

