# Epifly → Dokploy CLI / SDK refactor plan

> Move Epifly off the hand-rolled tRPC client in `dokploy/lib/trpc.mjs` and onto
> the **official Dokploy surface** — `@dokploy/sdk` for typed in-process calls
> and `@dokploy/cli` for operator-facing log / one-shot commands. Keep the SSH
> manager→worker fallback for log retrieval because the Dokploy backend still
> does not expose streaming Docker-service logs on the version we run.

---

## 0. Why refactor

Current state:

- `dokploy/lib/trpc.mjs` is a hand-rolled tRPC HTTP client (~80 LOC) that we
  invoke from both `dokploy/epifly-deploy/scripts/deploy.mjs` (orchestrator
  inside the cluster) and `tools/epifly/src/**` (operator CLI on the laptop).
- Every call passes `x-api-key`, JSON-encodes `{ json: input }` into the URL
  for queries and the body for mutations, then peels `result.data.json` back
  off the wire envelope.
- `epifly logs` tries `deployment.logs` first, fails on our Dokploy version,
  then falls back to multi-host SSH + Docker service/container logs and a
  manager-relay to the worker node.

Pain points:

1. We track Dokploy procedure names by hand. When Dokploy renames a procedure
   (e.g. `environment.update` → `compose.update`), our CLI breaks silently.
2. There is no central type for any input/output — every consumer casts to
   `any` and reads optional fields defensively.
3. `deployment.logs` is the wrong endpoint and we cannot easily discover what
   the right one is from inside the codebase.
4. Operator UX is split: anything Epifly does not wrap requires curling the
   tRPC URL by hand. The official `dokploy` CLI already covers all 449
   procedures.

Target state:

- `dokploy/lib/trpc.mjs` is replaced by `@dokploy/sdk` calls (typed, versioned,
  auto-generated from `openapi.json`).
- `tools/epifly` keeps its commands but its API layer becomes a thin wrapper
  around `@dokploy/sdk`.
- Operators get `dokploy …` as a first-class escape hatch for everything not
  in `epifly …`, with the same credentials.
- `epifly logs` still owns SSH/worker-node log retrieval (because that lives
  outside the Dokploy API), but it uses the SDK for compose/service lookup.

---

## 1. Inventory of the existing surface

Every place the hand-rolled client is used today. Each must be migrated or
explicitly justified to keep.

### 1.1 `dokploy/lib/trpc.mjs` consumers (zero-dep, runs in `node:22-alpine`)

| File | Procedures used |
|---|---|
| [dokploy/epifly-deploy/scripts/deploy.mjs](../epifly-deploy/scripts/deploy.mjs) | `compose.search`, `compose.one`, `compose.create`, `compose.update`, `compose.deploy`, `project.one`, `project.update`, `environment.one` |
| [dokploy/scripts/sync-domains.mjs](../scripts/sync-domains.mjs) | `domain.byApplicationId`, `domain.create`, `domain.update`, `domain.delete`, `compose.search` |
| [dokploy/test-dokploy-api.mjs](../test-dokploy-api.mjs) | `environment.one`, `project.one` |

### 1.2 `tools/epifly` consumers (TS, transpiled with `tsup`)

| File | Procedures used |
|---|---|
| [tools/epifly/src/commands/init.ts](../../tools/epifly/src/commands/init.ts) | `environment.list` |
| [tools/epifly/src/commands/deploy.ts](../../tools/epifly/src/commands/deploy.ts) | `compose.search`, `compose.one`, `compose.update`, `compose.deploy` |
| [tools/epifly/src/commands/logs.ts](../../tools/epifly/src/commands/logs.ts) | `compose.search`, `deployment.logs` (fails) |
| [tools/epifly/src/commands/status.ts](../../tools/epifly/src/commands/status.ts) | `compose.search` |
| [tools/epifly/src/commands/diff.ts](../../tools/epifly/src/commands/diff.ts) | `environment.one`, `project.one` |
| [tools/epifly/src/commands/secret.ts](../../tools/epifly/src/commands/secret.ts) | `environment.one`, `project.one`, `project.update` |
| [tools/epifly/src/commands/doctor.ts](../../tools/epifly/src/commands/doctor.ts) | `project.all`, `environment.one`, `compose.search`, `project.one` |
| [tools/epifly/src/lib/log-tail.ts](../../tools/epifly/src/lib/log-tail.ts) | `compose.one`, `deployment.logs` (fails) |

### 1.3 What the official Dokploy surface gives us

- **`@dokploy/sdk`** (typed, OpenAPI-generated, MIT) — 524 endpoints. Same
  auth header (`x-api-key`). Returns `{ data, error }` per call.
  - `client.setConfig({ baseUrl, headers })`
  - `composeAll`, `composeOne`, `composeCreate`, `composeUpdate`,
    `composeDeploy`, `projectOne`, `projectUpdate`, `environmentOne`,
    `environmentAll`, `domainCreate`, `domainByCompose`, …
- **`@dokploy/cli`** (`npm i -g @dokploy/cli`) — 449 commands. Auth via
  `dokploy auth -u URL -t TOKEN` or `DOKPLOY_URL` / `DOKPLOY_API_KEY` env.
  Shape: `dokploy <group> <action> [--flag value] [--json]`. Useful for
  operators outside Epifly automation.

### 1.4 What neither covers (still needs SSH)

- Streaming Docker service / container logs on a Swarm worker node from a
  laptop that cannot reach the worker directly.
- Reading `/var/run/docker.sock` for the volume bootstrap phase
  (`dokploy/lib/docker.mjs`).

These stay on SSH + Docker Engine API. The plan never removes that path.

---

## 2. Constraints

1. **Orchestrator stays zero-dep at runtime.** `deploy.mjs` runs inside
   `node:22-alpine` with no `pnpm install` step. We have two options:
   - (a) Bundle `@dokploy/sdk` into a single ESM file with `tsup` at build
     time and bake it into the image, OR
   - (b) Add `pnpm install --prod --filter epifly-deploy` to the Dockerfile.
     Option (a) keeps the existing "drop a `.mjs` file in" deployment model.
2. **No new long-lived credentials.** Reuse `DOKPLOY_URL` + `DOKPLOY_API_KEY`
   exactly as today.
3. **SSH key path stays unchanged.** No changes to `~/.ssh/config`, no new
   keys provisioned. `--jump-host` / `--host` flags keep their semantics.
4. **Test coverage cannot regress.** All 141 Jest tests must still pass; we
   add SDK-mock tests for the new wrapper.
5. **`epifly logs` must keep working against the current cluster** where
   Dokploy logs streaming is not available.

---

## 3. Target architecture

```
┌──────────────────────────┐   ┌──────────────────────────┐
│  tools/epifly (CLI, TS)  │   │  deploy.mjs (orchestr.)  │
└─────────────┬────────────┘   └─────────────┬────────────┘
              │                              │
              ▼                              ▼
       ┌─────────────────────────────────────────┐
       │  dokploy/lib/dokploy-client.{mjs,ts}    │  ← thin wrapper
       │  • configureClient(cfg) (sets x-api-key)│
       │  • re-exports SDK operations we use     │
       │  • optional: unwrap({data,error}) → val │
       └────────────────────┬────────────────────┘
                            │
                            ▼
                ┌──────────────────────┐
                │   @dokploy/sdk       │  ← npm, OpenAPI-generated
                └──────────────────────┘

           (operator escape hatch, not invoked from code)
                ┌──────────────────────┐
                │   @dokploy/cli       │  ← npm i -g @dokploy/cli
                └──────────────────────┘

       ┌────────────────────────────────────────────────┐
       │  tools/epifly/src/lib/ssh.ts (UNCHANGED)       │
       │   used by `epifly logs` and `epifly wipe`      │
       └────────────────────────────────────────────────┘
```

Decision: **prefer SDK over shelling out to the CLI from Epifly code.**
Shelling out adds process overhead, JSON-parse-stdout fragility, and a global
install requirement on every operator machine. The CLI stays a documented
operator tool, not an internal dependency.

---

## 4. Step-by-step migration plan

Each phase is independently shippable. Tests and the live cluster keep
working between phases.

### Phase 1 — Spike & decisions (no production change)

1. Add `@dokploy/sdk` as a dev dep in the repo root and confirm:
   - `pnpm add -D -w @dokploy/sdk`
   - write a 20-line script in `dokploy/scripts/sdk-probe.mjs` that calls
     `environmentOne`, `projectOne`, `composeAll` against beta and prints
     the typed responses.
2. Confirm which SDK function corresponds to each of the 12 procedures listed
   in §1.1 / §1.2. Record the mapping in this doc (table appended in §8).
3. Confirm SDK behavior for the missing `deployment.logs`:
   - try `deploymentLogs`, `applicationReadTraefikConfig`,
     `dockerContainerLogs`, etc., and document which (if any) actually
     streams. The expectation is none of them work for compose services on
     our Dokploy version — confirm that before designing around it.
4. Install `@dokploy/cli` globally on one operator laptop and run
   `dokploy compose all`, `dokploy compose one --composeId …`,
   `dokploy compose deploy --composeId …` to confirm the operator UX.

Exit criteria: a recorded mapping table + a yes/no on streaming logs.

### Phase 2 — Build the wrapper module

Goal: one place to configure the SDK, one place to unwrap `{ data, error }`.

1. Add `dokploy/lib/dokploy-client.mjs` (zero runtime deps when bundled):

   ```js
   import { client, ...ops } from "@dokploy/sdk";

   export function configureClient({ baseUrl, apiKey }) {
     client.setConfig({
       baseUrl: `${baseUrl.replace(/\/+$/, "")}/api`,
       headers: { "x-api-key": apiKey },
     });
   }

   export async function call(op, input) {
     const { data, error } = await op(input);
     if (error) {
       const msg = error?.message ?? JSON.stringify(error);
       throw new Error(`${op.name}: ${msg}`);
     }
     return data;
   }

   export * as sdk from "@dokploy/sdk";
   ```

2. Add a TS-friendly re-export at `tools/epifly/src/lib/dokploy.ts` that
   imports from `../../../../dokploy/lib/dokploy-client.mjs` and re-exports
   `configureClient`, `call`, and the SDK namespace. This is the only file
   `tools/epifly/src/commands/*.ts` imports for Dokploy access.
3. Add unit tests:
   - `tests/unit/dokploy-client.test.ts` mocks one SDK operation and asserts
     `call(op, input)` throws on `error`, returns `data` on success, and
     preserves the operation name in the error message.

### Phase 3 — Migrate `tools/epifly` (operator CLI)

Order matters: do read-only commands first, then mutating ones, then `deploy`.

1. **`status`** (`compose.search` only): swap to `sdk.composeAll` (or the
   equivalent search operation from the mapping). Re-run
   `pnpm --filter @conusai/epifly test:all`.
2. **`doctor`**: replace `project.all`, `environment.one`, `compose.search`,
   `project.one`. Keep the same `CheckResult[]` output.
3. **`diff`** and **`secret`** (read project env, write project env):
   - read via `sdk.projectOne` + `sdk.environmentOne`.
   - write via `sdk.projectUpdate({ body: { projectId, env } })`.
   - The body shape may differ from `{ projectId, env }` — confirm in §8 and
     adjust callers.
4. **`init`**: replace `environment.list` with `sdk.environmentAll`.
5. **`deploy`**: replace `compose.search` / `compose.one` / `compose.update`
   / `compose.deploy`. The post-deploy guard (`ensureManagedServicesRunning`)
   keeps its logic; only the underlying call site changes.
6. **`logs`** and **`log-tail`**: keep the SSH/worker-relay path unchanged.
   Replace the failing `deployment.logs` attempt with the SDK equivalent if
   Phase 1 found one; otherwise drop the attempt entirely and go straight to
   SSH fallback (the API attempt was always wasted on our Dokploy version).
7. Delete `dokploy/lib/trpc.mjs` imports from `tools/epifly/src/**` and
   confirm the file is no longer referenced from TS code.

Exit criteria: `pnpm --filter @conusai/epifly test:all` and
`pnpm --filter @conusai/epifly build` both green; smoke-test
`epifly status`, `epifly diff`, `epifly logs <app>`, `epifly deploy --dry-run`
against beta.

### Phase 4 — Migrate orchestrator (`deploy.mjs`) and helpers

This is the riskier phase because the orchestrator runs inside the Dokploy
container and a bad release blocks all deploys.

1. Add a tsup target that bundles `dokploy/lib/dokploy-client.mjs` + the
   subset of `@dokploy/sdk` it touches into a single `dist/dokploy-client.mjs`
   shipped alongside the orchestrator. Update the Dockerfile to bind-mount
   or COPY that bundle.
2. Update `dokploy/epifly-deploy/scripts/deploy.mjs` Phase 1, 2, 3, 4 calls
   to use the wrapper. Behaviour must be byte-identical against beta.
3. Update `dokploy/scripts/sync-domains.mjs` the same way. This one runs in
   init-containers for every app; verify the bundle is reachable from those
   compose files too.
4. Update `dokploy/test-dokploy-api.mjs` (or delete it — it is a one-off
   probe that the new wrapper makes obsolete).
5. Delete `dokploy/lib/trpc.mjs` once nothing imports it.
6. Tag a `beta`-only release first. Run `epifly deploy --dry-run` then a
   real `epifly deploy`. Watch `epifly logs epifly-deploy` for the full
   Phase 0–5 sequence to complete.

Exit criteria: end-to-end `epifly deploy` succeeds on beta with the new
orchestrator; verify checks pass; rollback path documented (keep the prior
image tag, redeploy if Phase 4 misbehaves).

### Phase 5 — Expose `@dokploy/cli` to operators

Pure documentation / convenience phase. No code changes inside Epifly.

1. Add a short "Dokploy CLI" section to `dokploy/README.md`:
   - install: `pnpm dlx @dokploy/cli@<pinned-version> --help` or
     `npm i -g @dokploy/cli`.
   - auth: reuse the same `DOKPLOY_URL` + `DOKPLOY_API_KEY` already in
     `dokploy/.dokploy`. Either `dokploy auth -u $DOKPLOY_URL -t $DOKPLOY_API_KEY`
     or `set -a; source dokploy/.dokploy; set +a`.
   - common recipes: `dokploy compose all`,
     `dokploy compose one --composeId …`,
     `dokploy compose deploy --composeId … --json`,
     `dokploy project all`, `dokploy server all`.
2. Pin the CLI version in `dokploy/README.md` so it tracks our Dokploy
   server version (e.g. server 0.29.x → CLI `@dokploy/cli@0.29.x`).
3. Add a `just dokploy-cli <args…>` recipe in the root `justfile` that
   sources `dokploy/.dokploy` and shells out to `pnpm dlx @dokploy/cli@…`,
   so operators do not need a global install.

Exit criteria: `just dokploy-cli compose all --json` works from a fresh
checkout without manual env setup.

### Phase 6 — Decide what `epifly logs` should do long-term

Two paths, pick one based on Phase 1's answer about API log streaming.

- **6a (Dokploy API exposes logs)**: drop SSH fallback, simplify `logs.ts`
  to a single SDK call. Keep `--jump-host` only behind `--ssh`.
- **6b (Dokploy API still does not expose logs)**: keep current behavior;
  document the manager-relay path in `dokploy/docs/arch.md` so the next
  operator does not re-discover the worker-node topology by reading commit
  history. This is the most likely outcome.

---

## 5. Test plan

For every phase above:

1. `pnpm --filter @conusai/epifly test:all` — must stay green.
2. `pnpm --filter @conusai/epifly check-types` — must stay green.
3. `pnpm --filter @conusai/epifly build` — single-file bundle must build.
4. `node tools/epifly/dist/epifly.mjs status` against beta — smoke test.
5. `node tools/epifly/dist/epifly.mjs logs web -n 50 --host root@10.66.66.2 --jump-host root@beta.test.cloud.conusai.com`
   — confirm worker-node logs still work.
6. `node tools/epifly/dist/epifly.mjs deploy --dry-run` against beta — must
   produce the same phase output as before the refactor.

New tests added in Phase 2:

- `tools/epifly/tests/unit/dokploy-client.test.ts` — verifies the `call`
  wrapper unwraps `{ data, error }` correctly.
- `tools/epifly/tests/unit/sdk-mapping.test.ts` — small smoke that every
  SDK op named in the §8 mapping table is actually importable from
  `@dokploy/sdk`.

---

## 6. Rollback strategy

- Phases 2 + 3 are local to the operator CLI; rollback is `git revert` +
  rebuild + `pnpm --filter @conusai/epifly link --global`. Production is
  untouched.
- Phase 4 touches the orchestrator. Rollback strategy:
  1. Keep `dokploy/lib/trpc.mjs` and the old call sites on a branch.
  2. Deploy the orchestrator change to beta only first.
  3. If beta deploys break, redeploy the prior `epifly-deploy` compose
     (Dokploy keeps the last image around; `compose.rollback` is in the SDK
     as `rollbackRollback` — confirm exact name in §8).

---

## 7. Risks & mitigations

| Risk | Mitigation |
|---|---|
| `@dokploy/sdk` version drifts from our Dokploy server version | Pin `@dokploy/sdk` to the exact server release; add a `doctor` check that calls `settingsGetVersion` (or whichever op exposes it) and warns on mismatch. |
| SDK adds a runtime dep the orchestrator image cannot install | Bundle with `tsup` in Phase 4 so the orchestrator stays "drop a `.mjs` file in". |
| OpenAPI rename breaks the wrapper | Caught by Phase 2 unit test (`sdk-mapping.test.ts`) — fails fast on `pnpm install` after an SDK bump. |
| Dokploy CLI version skew on operator laptops | Phase 5 pins the version via `pnpm dlx @dokploy/cli@<version>`; operators never have to think about it. |
| `deploy.mjs` regression takes down beta deploys | Phase 4 ships behind a feature flag (`DEPLOY_USE_SDK=1`) for one cycle, then becomes default. |

---

## 8. SDK ↔ tRPC procedure mapping (to fill in during Phase 1)

| Current tRPC procedure | `@dokploy/sdk` export | Status |
|---|---|---|
| `project.all` | `projectAll` | TBD |
| `project.one` | `projectOne` | TBD |
| `project.update` | `projectUpdate` | TBD |
| `environment.all` / `environment.list` | `environmentAll` | TBD |
| `environment.one` | `environmentOne` | TBD |
| `compose.search` | `composeAll` (filtered) | TBD |
| `compose.one` | `composeOne` | TBD |
| `compose.create` | `composeCreate` | TBD |
| `compose.update` | `composeUpdate` | TBD |
| `compose.deploy` | `composeDeploy` | TBD |
| `domain.byApplicationId` | `domainByApplicationId` or `domainByCompose` | TBD |
| `domain.create` / `update` / `delete` | `domainCreate` / `domainUpdate` / `domainDelete` | TBD |
| `deployment.logs` (currently fails) | `deploymentLogs` if it exists | TBD |

Fill the third column in Phase 1 before starting Phase 2. Anything that
ends "does not exist" needs an explicit decision (drop, replace with SSH,
or open an upstream issue).

---

## 9. Out of scope

- Replacing SSH-based wipe (`tools/epifly/src/commands/wipe.ts`). That stays
  on `ssh + /opt/epifly/scripts/wipe-volumes.sh`.
- Replacing the Docker Engine API calls in `dokploy/lib/docker.mjs`. Those
  are not Dokploy API calls and have nothing to do with the SDK.
- Replacing `dokploy/scripts/sync-domains.mjs`'s YAML parser — orthogonal.
- Migrating `apps/**` to the SDK. Only `dokploy/**` and `tools/epifly/**`
  are in scope.

---

## 10. Definition of done

- `dokploy/lib/trpc.mjs` deleted.
- All Dokploy HTTP calls go through `dokploy/lib/dokploy-client.mjs` (which
  wraps `@dokploy/sdk`).
- `epifly status | diff | doctor | deploy | logs | secret | init` all work
  against beta, with green tests and a successful end-to-end deploy.
- `dokploy/README.md` documents `@dokploy/cli` as the operator escape hatch
  for anything Epifly does not wrap, with a pinned version.
- `dokploy/docs/arch.md` updated to note that Epifly's data plane is the SDK,
  not a bespoke tRPC client.
