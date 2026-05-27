## Verdict

The plan is **technically strong**, but the UX model is half-baked. It solves backend projection well: redb remains source of truth, Markdown becomes a workspace-facing read model, Qdrant gets stable anchors, and the UI can finally show conversations beside documents. That part is sane. 

But your hierarchy instinct — **Earth → Continent → Country → City → Address → Building → Room** — exposes the missing layer: users do not think in “thread projections.” They think in **places, projects, tasks, and artifacts**. Calling this a file/thread management system without defining those levels is how you get a beautiful backend and a UI that feels like a filing cabinet designed by a database.

## The better mental model

Use this hierarchy:

```text
Workspace
  → Space / Project
    → Area / Folder
      → Thread
        → Turn / Decision / Artifact
```

For example:

```text
ConusAI
  → OrdeON.
    → Restaurants
      → KEBAB Inn
        → Onboarding thread
        → Contract PDF
        → Menu import results
        → Payment setup notes
```

That is closer to how people work. They do not wake up thinking: “I need the deterministic Markdown projection node.” Shocking, I know.

The user-facing structure should be:

```text
Workspace = organization / tenant
Project = meaningful work context
Folder = loose grouping
Thread = conversation as work history
Artifact = file, note, generated doc, decision, upload
```

Backend can still store it as nodes. UX should not expose the machinery.

## What is good in your current plan

### 1. Projection pattern is correct

Keeping **redb as source of truth** and projecting threads into Markdown workspace nodes is the right split. Threads are event logs. Markdown is a readable artifact. Mixing those responsibilities would be architectural soup. Your plan explicitly avoids moving thread source-of-truth to RustFS/Markdown and explains why redb is better for append, concurrency, monotonic sequence numbers, and low-latency streaming. 

Keep this.

### 2. Deterministic `node_id` is a good MVP decision

The `blake3(tenant_id ‖ thread_id)` approach gives you idempotent upsert without creating a second lookup table. That is boring in the best way. Boring systems survive production.

But there is one correction: your own risk section says renamed nodes should be preserved by storing `projection_node_id` in thread metadata. That should not be Phase 4-ish. It should be **v1**. Otherwise deterministic IDs will constantly fight user intent.

### 3. Markdown body shape is decent

YAML frontmatter + deterministic turn headings is good because it supports human scanning and chunking. The plan says `## Turn N · role · timestamp` headings let the heading-aware chunker split cleanly. That is exactly the kind of boring leverage you want. 

However, tool calls inside Markdown are dangerous. Useful for search, yes. But if arguments can contain secrets, PII, tokens, internal URLs, payment metadata, etc., redaction cannot be “remembered later.” It must be mandatory before v1.

## Main UX flaw

The current plan makes **threads look like files**.

That is useful for search and export, but dangerous for navigation. A thread is not just a file. It has temporal flow, participants, decisions, generated artifacts, tool calls, and follow-up actions.

So the UI should not show projected conversations as ordinary `.md` files by default. It should show them as a separate node type:

```ts
kind: "thread"
storageKind: "markdown_projection"
sourceOfTruth: "redb"
```

Physically, yes, it can reuse file storage. Conceptually, no. Do not make users infer that a conversation is just a Markdown document. That is leaky abstraction cosplay.

## Better MVP backend model

Keep your implementation mostly intact, but adjust the domain model.

### Current plan

```text
workspace_node.kind = "file"
mime_type = "text/markdown"
metadata.thread_id = ...
metadata.projection.version = 1
```

### Better plan

```text
workspace_node.kind = "thread"
mime_type = "text/markdown"
metadata.source = "thread_projection"
metadata.thread_id = ...
metadata.projection.version = 1
metadata.projection.readonly = true
```

Then UI can render:

```text
💬 Reconcile Q1 invoices
📄 invoice.pdf
📝 generated-summary.md
📊 q1-expenses.xlsx
```

Still one tree. Better semantics. Less user confusion.

## Hierarchy recommendation

Your Earth → Continent → Country example is useful, but do not overfit it. Humans like hierarchy only until it becomes archaeology. Miller’s classic work introduced chunking limits around short-term memory, and later interpretations emphasize that people handle information better when grouped into meaningful chunks, not endless flat lists. ([labs.la.utexas.edu][1])

So for MVP:

```text
Tenant / Workspace
  Project
    Folder / Collection
      Item
```

Where `Item` can be:

```text
file
thread
note
generated artifact
external link
```

Do **not** go deeper unless users create the depth themselves.

Bad:

```text
Workspace → Department → Project → Client → Month → Type → Thread → Turn
```

That is not UX. That is sediment.

Good:

```text
Workspace → Project → Items
```

Then add filters:

```text
Type: Files / Threads / Notes
Time: Today / This week / Month
People: Me / Shared
Status: Active / Archived
```

This matters because information architecture should match users’ mental models; card sorting is specifically used to uncover how users naturally group content, while tree testing validates whether users can actually find things in the proposed hierarchy. ([Nielsen Norman Group][2])

## Best MVP structure

Use **tree + filters**, not tree-only.

Tree-only fails because one object can belong to multiple mental categories:

```text
"KEBAB Inn onboarding"
```

Could belong under:

```text
Restaurants
Sales
Lithuania
Payments
May 2026
```

If you force exactly one path, users lose things. NN/g calls this a polyhierarchy problem: placing one item in more than one IA category can support different mental models, but should be used carefully. ([Nielsen Norman Group][3])

For MVP, do not implement full polyhierarchy. Use:

```text
Primary location: one workspace path
Secondary access: tags, search, filters
```

Backend:

```ts
workspace_node {
  id
  parent_id
  kind: "file" | "folder" | "thread"
  name
  path
  tags[]
  source_type
  source_id
  created_at
  updated_at
}
```

Thread projection metadata:

```ts
thread_projection {
  thread_id
  node_id
  last_seq
  content_hash
  projected_at
  message_count
  status: "active" | "archived" | "paused"
}
```

Yes, this adds a tiny table. No, deterministic IDs alone are not enough once users rename/move/pause/archive. Fighting future UX to save one table is how MVPs become landfill.

## Recommended UX behavior

### Workspace tree

Show threads, but visually distinct:

```text
📁 Conversations
  📁 2026-05
    💬 Reconcile Q1 invoices
    💬 KEBAB Inn onboarding
```

Not:

```text
Reconcile Q1 invoices.md
Kebab Inn onboarding.md
```

The `.md` suffix is developer residue. Hide it.

### Thread header

Show:

```text
Saved in: Conversations / 2026-05 / Reconcile Q1 invoices
[Open in workspace]
```

Good.

But do not show it too aggressively. Users came to chat, not to admire your projection pipeline.

### Search results

Group by intent:

```text
Conversations
Documents
Generated files
```

Your Phase 3 already says search should group `kind=thread` separately from `kind=file`. Good. Move that from nice-to-have into MVP. 

### Delete behavior

Your current plan says deleting projected MD does not delete the conversation and the next projection recreates it. Technically correct, UX-hostile.

This will feel haunted, as your own risk table admits. 

Better:

When user deletes projected thread node:

```text
- keep redb thread
- set projection.status = "paused"
- hide from workspace
- allow "Restore to workspace"
```

Then if the user continues the chat, show:

```text
This conversation is hidden from workspace.
[Restore]
```

Do not silently resurrect it. Ghost files are funny exactly once.

## Backend changes I would make before v1

### 1. Add explicit `kind = "thread"`

Do not overload `"file"`.

```rust
enum WorkspaceNodeKind {
    Folder,
    File,
    Thread,
}
```

If changing enum is expensive, use metadata for now:

```json
{
  "kind": "file",
  "semantic_kind": "thread"
}
```

But that is weaker. MVP does not mean “confuse yourself early.”

### 2. Add `thread_projection` index/table

Even with deterministic `node_id`, you need state:

```rust
ThreadProjection {
    tenant_id,
    thread_id,
    node_id,
    status, // active | paused | error
    last_seq,
    content_hash,
    projected_at,
    folder_path,
}
```

This solves:

```text
rename preservation
delete/pause behavior
projection health
admin reproject
debugging
```

Your plan currently tries to avoid an extra table. Cute. Add the table.

### 3. Make redaction mandatory

Before rendering tool calls:

```rust
redact_tool_args(args)
redact_tool_result(result)
```

Never embed raw tool payloads into Markdown. The plan already mentions this in risks, but risk mitigation is not implementation. Put it in Phase 1.

### 4. Do not index all tool details by default

Search should find conversations by user intent and assistant answer, not by every internal tool parameter.

Recommended Markdown:

```markdown
<details>
<summary>Tool activity</summary>

- Searched workspace files
- Read invoice.pdf
- Extracted total amount

</details>
```

Full raw JSON stays in redb.

### 5. Add max-turn policy immediately

Your risk section says long threads may produce huge Markdown bodies and proposes truncation to recent N turns + summary stub. 

That should be v1:

```json
{
  "thread_export": {
    "max_turns": 200,
    "include_tool_details": false
  }
}
```

Otherwise your first power user creates a 4 MB Markdown blob and everyone pretends this was unpredictable.

## Revised MVP rollout

### Phase 1 — Backend projection, but with correct semantics

Must include:

```text
- Workspace node semantic kind: thread
- thread_projection state table/index
- deterministic fallback node_id
- redaction before Markdown render
- max_turns default
- projection pause on delete
- skip-if-unchanged
- GET /threads/{id}/projection
```

### Phase 2 — Replace old thread indexing

Good as written:

```text
- remove spawn_index_job
- index projected Markdown body
- Qdrant chunks tied to projection node_id
```

Your criticism of old `spawn_index_job` is correct: indexing snippets under the active workspace node creates orphaned or misleading search results. 

### Phase 3 — Minimal UI

MVP UI should include only:

```text
- Thread appears in workspace tree with chat icon
- Chat header has "Open in workspace"
- Search groups conversations separately
- Delete hides projection, does not delete redb thread
```

Do not add fancy archive automation, per-day splits, sidecar conflict UI, export derivatives, or admin ceremony yet. That is not MVP; that is product manager taxidermy.

## Final recommended architecture

```text
redb
  threads
  messages
  thread_projection_index

WorkspaceStore
  workspace_nodes
    kind = folder | file | thread
    source_type = upload | generated | thread_projection
    source_id = thread_id when thread

RustFS
  rendered markdown body

Qdrant
  chunks indexed under workspace_node.node_id

UI
  tree uses semantic kind
  search groups by kind
  thread opens either as chat or readable document
```

## Final judgment

Keep the projection idea. It is the right backend move.

But change the product model from:

```text
"threads become Markdown files"
```

to:

```text
"threads become first-class workspace items with a Markdown-readable surface"
```

That one distinction saves the UX. Files and threads can share infrastructure, but they should not share identity. Users understand places, projects, conversations, and artifacts. Build around that hierarchy, not around your storage implementation.

[1]: https://labs.la.utexas.edu/gilden/files/2016/04/MagicNumberSeven-Miller1956.pdf?utm_source=chatgpt.com "The Magical Number Seven, Plus or Minus Two"
[2]: https://www.nngroup.com/articles/card-sorting-definition/?utm_source=chatgpt.com "Card Sorting: Uncover Users' Mental Models"
[3]: https://www.nngroup.com/videos/polyhierarchy-information-architecture/?utm_source=chatgpt.com "Polyhierarchy in Information Architecture (Video) - NN/G"

Yes — but **not as “store later and hope.”** That is how you build a fast system that occasionally loses user data. Very innovative. Very bankrupt.

The correct pattern is:

```text
Write to memory first for speed
Append to durable log immediately for reliability
Process heavy storage/indexing asynchronously
```

So: **in-memory for hot runtime state**, **durable append log for truth**, **async workers for projection/search/files**.

## The right architecture

```text
Chat request
  ↓
In-memory ThreadRuntime
  - active stream state
  - message buffer
  - tool-call state
  - temporary title/context
  ↓
Fast durable append
  - redb / Postgres / WAL-style event store
  - append user message
  - append assistant deltas or final message
  ↓
Async background jobs
  - project thread to workspace node
  - write Markdown body
  - chunk + embed
  - update Qdrant
  - emit realtime workspace update
```

The key is this:

```text
Memory is cache.
Durable event log is source of truth.
Async projection is read model.
```

Do **not** make memory the source of truth unless you enjoy debugging “where did my conversation go?” tickets.

## What should be in memory

Use memory for things that are needed during the active request:

```rust
ThreadRuntime {
    tenant_id,
    thread_id,
    active_run_id,
    pending_user_message,
    assistant_stream_buffer,
    tool_call_state,
    cancellation_token,
    last_seen_seq,
}
```

This gives better performance because the streaming path does not constantly rebuild state from storage.

Good candidates:

```text
active SSE stream state
current assistant response buffer
tool call progress
temporary context
debounce/coalescing guards
recent thread cache
projection queue state
```

Bad candidates:

```text
only copy of messages
only copy of tool results
only copy of user uploads
only copy of payment/order data
```

If losing it would make the user angry, it does not belong only in memory. Revolutionary stuff.

## What should be persisted synchronously

Persist the minimum durable facts immediately:

```text
user message accepted
assistant message completed
tool call started/completed if important
thread metadata updated
message sequence number
```

For chat reliability, I would persist:

```text
1. user message before model call
2. assistant final message after completion
3. tool call summaries/results if they affect final answer
```

If you want stronger recovery during streaming, persist assistant deltas in batches:

```text
every 500–1000 ms
or every N tokens
or on sentence boundary
```

But for MVP, final-message persistence is probably enough unless your users care about recovering half-written answers after crashes.

## What should be async

These are async jobs:

```text
Markdown projection
workspace node update
RustFS body write
Qdrant embedding
search indexing
audit enrichment
realtime workspace refresh
summary generation
title generation
```

Your existing projection plan already moves the thread into the workspace tree after assistant `done`, uses `tokio::spawn`, coalesces per thread, and skips unchanged content with a hash. That is directionally correct. 

But plain `tokio::spawn` is not enough for reliability.

## The weak part in the current plan

This part is fragile:

```rust
tokio::spawn(project_thread(...));
```

If the process crashes after the assistant message is persisted but before projection finishes, the projection is lost unless you have a repair/backfill job.

Your plan has manual reproject endpoints, which helps, but that is admin duct tape. Useful, not sufficient. 

Better:

```text
persist assistant message
persist projection job record
worker claims job
worker projects thread
worker marks job complete
```

That gives crash recovery.

## Better MVP design

Add a tiny durable job/outbox table.

```rust
ProjectionJob {
    job_id,
    tenant_id,
    thread_id,
    reason,          // assistant_done | manual_reproject | backfill
    status,          // pending | running | done | failed
    attempts,
    last_error,
    created_at,
    updated_at,
}
```

Flow:

```text
assistant done
  ↓
persist assistant message
  ↓
upsert projection_job(thread_id, status=pending)
  ↓
return response / close SSE
  ↓
background worker processes pending jobs
```

This is the **outbox pattern**. Boring. Reliable. Exactly what you want.

## Recommended flow

```text
1. User sends message
2. Save user message to redb/Postgres immediately
3. Keep active run state in memory
4. Stream assistant response from memory buffer
5. On completion:
   - save assistant message
   - enqueue projection job
   - enqueue embedding/indexing indirectly via projection
6. Worker:
   - loads latest thread from durable store
   - renders Markdown
   - writes workspace node
   - writes RustFS body
   - chunks/embeds into Qdrant
   - marks job done
```

Important detail: the worker must always load the **latest thread state**, not the snapshot from when the job was created. Your plan already says this inside the projection guard, which is good. 

## Use coalescing, not infinite jobs

For chatty threads, do not create 20 projection jobs.

Use one pending job per thread:

```sql
UNIQUE(tenant_id, thread_id, job_type)
```

Then:

```text
if pending/running job exists:
  mark dirty = true
  update requested_at
else:
  create pending job
```

Worker logic:

```text
process latest thread state
if dirty changed during processing:
  run once more
else:
  complete
```

This gives you:

```text
no duplicate projection storms
no stale projections
no Qdrant spam
no race condition circus
```

## Memory architecture

Use an in-memory runtime registry:

```rust
DashMap<(TenantId, ThreadId), Arc<ThreadRuntime>>
```

Inside each runtime:

```rust
struct ThreadRuntime {
    lock: Mutex<()>,
    stream_state: RwLock<StreamState>,
    last_activity: AtomicTimestamp,
    cancellation: CancellationToken,
}
```

Then garbage collect idle runtimes:

```text
remove after 10–30 minutes idle
rebuild from durable store when needed
```

Do not try to keep every thread in memory forever. That is not a cache. That is a memory leak with ambition.

## Reliability levels

### Weak but fast

```text
memory → tokio::spawn async write
```

Fast, but can lose data. Do not use for messages.

### Acceptable MVP

```text
sync persist messages → async projection
```

Good baseline.

### Strong MVP

```text
sync persist messages → durable outbox job → async worker projection
```

This is what I recommend.

### Production-grade

```text
sync append event log → transactional outbox → worker pool → idempotent projections → retry/dead-letter
```

Probably later, unless this product is already handling serious customer data.

## What I would change in your plan

### Replace direct `tokio::spawn(project_thread)` with durable enqueue

Instead of:

```rust
tokio::spawn(project_thread(...));
```

Use:

```rust
projection_jobs.enqueue_or_touch(tenant_id, thread_id).await?;
```

Then a worker does:

```rust
loop {
    let job = projection_jobs.claim_next().await?;
    project_thread(job.tenant_id, job.thread_id).await;
}
```

You can still use `tokio::spawn` internally to wake the worker. But the job itself must exist durably.

### Keep projection idempotent

Your existing design already uses:

```text
deterministic node_id
content_hash
last_seq
skip-if-unchanged
```

Keep all of that. It is exactly what makes async projection safe. 

### Add recovery on startup

On app boot:

```text
find projection jobs where status = running and updated_at older than 2 minutes
set status = pending
```

Also optionally:

```text
find threads updated after last projection
enqueue projection
```

That covers crashes.

## Best MVP version

```text
In-memory:
  active thread runtime
  stream buffers
  projection guards
  short-lived cache

Synchronous durable write:
  user message
  assistant final message
  thread metadata
  projection job/outbox record

Async:
  title generation
  Markdown projection
  workspace node update
  Qdrant indexing
  realtime tree update
```

## Bottom line

Yes, use memory. But only as a **performance layer**, not as the truth.

The best architecture is:

```text
Memory for active work.
Durable append store for truth.
Outbox queue for reliable async work.
Projection model for UX/search.
```

That gives you speed without building a beautiful data-loss machine.
