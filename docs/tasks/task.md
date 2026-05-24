## Verdict

Your architecture is **strong but overextended**. The foundation is credible: Rust/Axum gateway, typed OpenAPI, capability abstraction, tenant-aware storage boundaries, Qdrant semantic routing, OTel/Prometheus, SvelteKit + Tauri shared UI. That is not amateur hour. The uploaded architecture explicitly defines a multitenant agent gateway, capability providers, semantic router, redb/Qdrant/RustFS persistence, shared UI packages, generated OpenAPI, audit logs, and observability. 

But “best community standards” in 2026 are no longer “can it call tools?” They are:

1. **Can it avoid calling the wrong tool?**
2. **Can it survive poisoned tools, poisoned documents, and poisoned MCP metadata?**
3. **Can it be evaluated continuously, not just demoed?**
4. **Can it fail safely under tenant, billing, storage, and model-provider pressure?**

Right now, the design is **architecturally ambitious**, but it needs sharper control planes, stronger agent security, and more boring production discipline. Boring is good. Boring pays invoices.

---

## What is already good

### 1. Capability abstraction is the right core idea

The “everything is a capability” model is directionally correct. Research on agent systems favors modular tool routing and structured/auditable interfaces because they separate language reasoning from specialized execution. Your design does exactly that with native, chain, MCP, WASM, and job-backed providers behind one `CapabilityProvider` interface.  ([arXiv][1])

This is better than hardcoding random “tools” into an agent prompt like it’s 2023 and we’re all still pretending prompt spaghetti is architecture.

### 2. Server-side tenancy is correctly placed

The doc says tenant isolation is enforced at redb/Qdrant/S3 boundaries, not merely in handlers. That is the right instinct. Handler-level tenant checks are where startups go to create very expensive legal events. 

### 3. Generated OpenAPI + typed SDK is good discipline

Generated OpenAPI, typed `@conusai/types`, and SDK consumption are solid. This reduces contract drift and makes external integrations less painful. Your route-table verification is also a good move because docs that lie are worse than no docs; at least no docs have the decency to be obviously useless. 

### 4. Observability is not an afterthought

Prometheus, OpenTelemetry, Jaeger, route-level metrics, tenant/request IDs, and capability/router metrics are all aligned with modern production expectations. Community practice around Rust backends also keeps circling back to OpenTelemetry, although people correctly complain it is still annoyingly complex. ([Reddit][2])

---

## Main architectural weaknesses

### 1. Tool sprawl will hurt agent reliability

You currently expose many capabilities and then use semantic routing to select top-K tools. That is reasonable, but it is also fragile. Reddit/AI-builder discussions increasingly warn that too many independent tools make the model spend cognition on tool choice instead of task completion. One recent LocalLLaMA discussion makes the exact point: more tools increases selection difficulty and drops accuracy. ([Reddit][3])

**Fix:** keep semantic routing, but add a second layer: **task-scoped tool profiles**.

Instead of:

```txt
user request → semantic router → top-K capabilities → agent loop
```

Use:

```txt
user request
→ intent class
→ task profile
→ allowed capability set
→ semantic router within that set
→ execution
```

Example profiles:

| Task profile        | Allowed tools                                       |
| ------------------- | --------------------------------------------------- |
| document extraction | OCR, invoice, CV, medical claim, classifier         |
| workspace editing   | file read/write, versions, search                   |
| email composition   | compose-email, report-md                            |
| admin/debug         | runtime-echo, capability test, reload               |
| upload pipeline     | sense-mime, convert-pdf, classifier, plan-on-upload |

Semantic routing across everything is elegant. Elegance is how bugs dress for court.

---

### 2. MCP is treated as an integration format, not a hostile supply chain

Your design supports remote MCP providers and has `CONUSAI_MCP_ALLOWED_HOSTS`, which is good. But 2025–2026 research has made MCP security ugly. Multiple papers now discuss tool poisoning, prompt injection through MCP metadata, and vulnerabilities in real MCP clients. ([arXiv][4])

The weak point is not only “malicious user input.” It is **malicious tool descriptions, malicious schemas, malicious tool responses, and changed remote server behavior**.

**Fixes to add:**

| Risk                             | Required improvement                                                        |
| -------------------------------- | --------------------------------------------------------------------------- |
| Tool poisoning                   | Sign capability manifests and pin remote MCP tool schema hashes             |
| Tool rug-pull                    | Store last-known schema hash and require admin approval on schema drift     |
| Prompt injection via tool output | Mark all tool output as untrusted data, never instruction text              |
| Overbroad MCP access             | Per-capability egress allowlist, not only global host allowlist             |
| Remote MCP compromise            | Capability-level kill switch + degraded routing status                      |
| Prompt/schema injection          | Static scanner for descriptions, schemas, examples, and hidden instructions |

Add this to the architecture as a required trust boundary:

```txt
Remote MCP server
→ schema fetch
→ signature/hash verification
→ static policy scan
→ admin approval if changed
→ registered capability
→ runtime sandbox/egress policy
```

Without this, “self-registering capabilities” becomes “self-registering attack surface.” Lovely feature, terrible obituary.

---

### 3. Agent evals are under-specified

The doc mentions an `evals` harness and scorecard types, but it does not define a serious evaluation regime.  Research on LLM-agent evaluation emphasizes planning, tool use, memory, reflection, and task-specific benchmarks, while RAG evaluation literature stresses factuality, retrieval quality, safety, and efficiency. ([arXiv][5])

You need evals for the actual system, not just the model.

Add these eval suites:

| Eval suite            | Measures                                           |
| --------------------- | -------------------------------------------------- |
| tool-selection eval   | Did router choose correct capability?              |
| tool-argument eval    | Did model produce valid/optimal JSON?              |
| refusal/security eval | Did it avoid unsafe tool use?                      |
| tenant-isolation eval | Did tenant A ever retrieve tenant B content?       |
| upload-pipeline eval  | Did file type → extraction → indexing → plan work? |
| RAG/retrieval eval    | recall@k, MRR, faithfulness, citation correctness  |
| latency/cost eval     | p50/p95 routing, model, tool-call, upload pipeline |
| regression eval       | compare every capability before deploy             |

Minimum practical setup:

```txt
/tests/evals/
  routing/
  tool_args/
  rag/
  security/
  tenant_isolation/
  uploads/
  cost_latency/
```

Each eval case should include:

```json
{
  "input": "...",
  "tenant": "tenant_a",
  "expected_capability": "invoice-processing",
  "forbidden_capabilities": ["storage-fs", "admin"],
  "expected_schema_valid": true,
  "max_latency_ms": 3000,
  "must_not_leak": ["tenant_b"]
}
```

This is not optional. In 2026, an agent platform without evals is just a confident slot machine.

---

### 4. redb is risky as the central production metadata store

redb is fine for embedded local state, edge, dev, desktop, or single-node deployments. But your architecture uses it for threads, messages, workspace metadata, audit events, tenant seeding, and encrypted IAM creds. 

That raises production questions:

| Concern                 | Why it matters                                                                 |
| ----------------------- | ------------------------------------------------------------------------------ |
| HA                      | What happens when the node dies?                                               |
| backups                 | How are point-in-time restores handled?                                        |
| migrations              | How are schema changes versioned?                                              |
| scaling                 | Does one embedded store become the platform bottleneck?                        |
| audit durability        | Audit logs need stronger guarantees than “it lives in the same embedded file.” |
| credential blast radius | Encrypted creds are still sitting in the same operational database.            |

**Better architecture:**

Use redb only for:

```txt
local cache
dev/test mode
edge/offline shell metadata
ephemeral route snapshots
```

Use Postgres for:

```txt
tenants
users
threads
messages
workspace metadata
audit log index
capability registry state
billing metadata references
```

Use object storage for:

```txt
large artifacts
uploaded files
generated files
audit event archive
```

Use Qdrant for vectors, but with strict tenant filtering.

The boring answer is Postgres. Yes, revolutionary.

---

### 5. Vector-store tenant isolation needs paranoia

The doc says Qdrant stores capability embeddings and content embeddings, with tenant filtering on content vectors.  This is acceptable only if tenant filtering is mandatory, tested, and impossible to bypass.

Security people increasingly criticize naive centralized RAG/vector architectures because they often bypass original access controls and create new leakage surfaces. ([TechRadar][6])

**Required improvement:**

Add a `TenantScopedVectorStore` wrapper so code cannot query content vectors without tenant context.

Bad:

```rust
vector_store.search(query, filter)
```

Better:

```rust
tenant_vector_store.for_tenant(tenant_id).search(query)
```

And add tests:

```txt
tenant_a document indexed
tenant_b query semantically matches document
expected: zero tenant_a results
```

Do not trust developer discipline here. Developers are just users with commit access.

---

### 6. Public `/metrics`, `/docs`, `/openapi.json` need environment-specific policy

The doc lists `/metrics`, `/docs`, and `/openapi.json` as public/no-auth.  That is convenient in dev and often wrong in production.

| Endpoint                       | Production recommendation                                                |
| ------------------------------ | ------------------------------------------------------------------------ |
| `/metrics`                     | internal network only or auth-gated                                      |
| `/docs`                        | disabled or admin-gated                                                  |
| `/openapi.json`                | public only if this is intended as external API contract                 |
| `/admin/capabilities/register` | never in public router mentally; even with token, isolate and rate-limit |

Prometheus metrics can leak route names, tenant patterns, error rates, internal services, model failures, and infrastructure topology. Swagger docs help attackers build a menu. Very hospitable. Too hospitable.

---

### 7. Capability self-registration needs provenance

Self-registration is great for plugins. It is also how you accidentally build npm with tool execution privileges.

Your doc says TOML manifests can expose tools and hot-reload through `notify`.  Add:

```txt
capability provenance:
  author
  signing key id
  signature
  schema hash
  permissions requested
  network egress requested
  storage scopes requested
  risk class
  approval status
```

Then enforce:

| Capability type             | Required approval                   |
| --------------------------- | ----------------------------------- |
| chain-only, no external I/O | automatic in dev, approval in prod  |
| native                      | signed build artifact required      |
| WASM                        | signed + sandbox limits             |
| MCP remote                  | signed + schema pin + egress policy |
| job-backed                  | signed + queue/resource limits      |

---

## Recommended target architecture

Use this instead of the current “capability registry + semantic router does everything” model:

```txt
Client
  ↓
API Gateway / Axum
  ↓
Identity + Tenant + Plan + Rate Limit
  ↓
Request Classifier
  ↓
Task Policy Profile
  ↓
Capability Router
  ↓
Execution Sandbox
  ↓
Tool Result Sanitizer
  ↓
Agent Loop
  ↓
Audit + Trace + Eval Event
  ↓
Response
```

Add these new internal components:

| Component                   | Purpose                                                   |
| --------------------------- | --------------------------------------------------------- |
| `TaskProfileRegistry`       | Maps task type → allowed capabilities                     |
| `CapabilityPolicyEngine`    | Decides whether a capability may run for tenant/plan/task |
| `ToolOutputSanitizer`       | Treats tool output as data, not instructions              |
| `CapabilityProvenanceStore` | Stores signatures, schema hashes, approval state          |
| `EvalEventSink`             | Records routing/tool/model outcomes for regression tests  |
| `TenantScopedVectorStore`   | Prevents unscoped vector queries                          |
| `AgentRiskClassifier`       | Marks tool calls as read/write/destructive/external       |
| `HumanApprovalGate`         | Required for destructive or external side-effect actions  |

---

## Priority fixes

### P0 — must fix before serious production

1. **Move durable metadata from redb to Postgres**, or document redb as single-node beta-only.
2. **Lock down MCP:** schema pinning, manifest signing, output sanitization, egress allowlists.
3. **Add task-scoped tool profiles** before semantic routing.
4. **Gate `/metrics`, `/docs`, admin registration, and debug endpoints in production.**
5. **Add tenant-isolation tests for Qdrant and RustFS.**
6. **Create golden evals for routing, tool arguments, RAG, and security.**

### P1 — should fix soon

1. Add capability risk levels: `read`, `write`, `external`, `destructive`, `admin`.
2. Add human approval for destructive/external actions.
3. Add circuit breakers per remote MCP/capability.
4. Add queue/backpressure for upload indexing and post-upload plans.
5. Add per-capability latency/cost budgets.
6. Add audit event export to object storage or append-only log.

### P2 — polish

1. Replace generic top-K semantic routing with hybrid routing: intent + profile + semantic ranker + reject option.
2. Add reranking for workspace search/RAG.
3. Add “why this capability was selected” traces.
4. Add capability versioning and rollback.
5. Add production runbooks: backup, restore, tenant deletion, key rotation, incident response.

---

## Research/community alignment score

| Area                       |  Score | Notes                                                              |
| -------------------------- | -----: | ------------------------------------------------------------------ |
| Modular agent architecture | 8.5/10 | Strong capability model; aligned with agent-system research.       |
| Tool calling reliability   | 6.5/10 | Needs task profiles and fewer exposed tools per turn.              |
| MCP/security posture       |   5/10 | Allowlist exists, but modern MCP threat model requires much more.  |
| Multitenancy               |   7/10 | Correct boundary idea; needs hard tenant-scoped APIs and tests.    |
| Storage architecture       | 5.5/10 | redb is the weak link for serious cloud production.                |
| Observability              |   8/10 | Good metrics/traces; add eval events and agent-specific traces.    |
| Evals                      |   4/10 | Harness exists, but no serious documented eval discipline.         |
| Frontend/shared UI         |   8/10 | Shared packages, token rules, reduced motion/a11y checks are good. |
| Production readiness       |   6/10 | Good skeleton, but too many “sharp knives in a drawer.”            |

Overall: **7/10 architecture, 5.5/10 production safety.**

The architecture is promising. The danger is that it is trying to be an agent platform, plugin platform, RAG platform, desktop shell, billing platform, and storage platform at once. That can work, but only if the control plane becomes stricter than the feature plane. Right now the feature plane is flexing; the control plane needs a gym membership.

[1]: https://arxiv.org/html/2601.01743v1?utm_source=chatgpt.com "AI Agent Systems: Architectures, Applications, and Evaluation"
[2]: https://www.reddit.com/r/rust/comments/12bm2pu/observability_made_easy_building_a_restful_api/?utm_source=chatgpt.com "Building a RESTful API with Actix Web and OpenTelemetry"
[3]: https://www.reddit.com/r/LocalLLaMA/comments/1rrisqn/i_was_backend_lead_at_manus_after_building_agents/?utm_source=chatgpt.com "I was backend lead at Manus. After building agents for 2 ..."
[4]: https://arxiv.org/abs/2603.22489?utm_source=chatgpt.com "[2603.22489] Model Context Protocol Threat Modeling and ..."
[5]: https://arxiv.org/html/2503.16416v2?utm_source=chatgpt.com "A Survey on Evaluation of LLM-based Agents"
[6]: https://www.techradar.com/pro/rag-is-dead-why-enterprises-are-shifting-to-agent-based-ai-architectures?utm_source=chatgpt.com "​​RAG is dead: why enterprises are shifting to agent-based AI architectures"
