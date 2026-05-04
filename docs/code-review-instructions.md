# AI Code Review Instructions: Next.js Frontend Application (FSD + shadcn/ui Monorepo)

**Version:** 1.0.0  
**Last Updated:** 2026-05-04  
**Applies To:** `apps/web/` (Next.js 16+ frontend in Turborepo 2.x monorepo)  
**Strictness Level:** MAXIMUM — Any deviation from these instructions invalidates the review.  

These instructions are **mandatory** for any AI agent (Claude, Grok, Cursor, etc.) performing code reviews on this Next.js frontend app that retrieves data via API (OpenAI-compatible streaming API from Rust Rig + Axum backend).

---

## 1. Pre-Review Mandatory Steps (DO NOT SKIP)

1. Read **all** of the following before touching any code:
   - Root `turbo.json`, `pnpm-workspace.yaml`, `package.json`
   - `apps/web/package.json`, `next.config.ts`, `tsconfig.json`, `biome.json`
   - `packages/types/` (all contracts)
   - `packages/ui/` (shadcn/ui setup)
   - Existing `app/layout.tsx`, `app/globals.css`, and at least 3 feature folders for reference patterns

2. Verify tooling:
   - Confirm Bun is used for all dev commands (`bun run dev`, `bun run lint`, `bun run build`)
   - Confirm Biome is the **only** linter/formatter (no ESLint, no Prettier)
   - Run `cd apps/web && bun run lint && bun run build` **before** starting review

3. Internalize the **exact** project style guide (community standards enforced):
   - **Feature-Sliced Design (FSD)** v2+ (https://feature-sliced.design/)
   - **shadcn/ui + Radix UI** monorepo setup (https://ui.shadcn.com/docs/monorepo)
   - **Next.js 16+ App Router + React 19 Compiler** best practices (official docs)
   - **Vercel AI SDK v6+** for all streaming/API data retrieval (https://ai-sdk.dev/docs)
   - **Turborepo 2.x** workspace rules
   - **KISS + YAGNI + SRP** (Single Responsibility Principle per feature)

---

## 2. File Structure — STRICT ENFORCEMENT (FSD)

The ONLY acceptable structure inside `apps/web/src/` is:

```
src/
├── app/                          # Next.js App Router (Server Components by default)
│   ├── layout.tsx
│   ├── page.tsx
│   ├── loading.tsx
│   ├── error.tsx
│   └── [feature-routes]/
├── features/                     # Business domains (ONE folder = ONE feature)
│   ├── user-auth/
│   ├── data-dashboard/
│   ├── api-data-retrieval/       # ← Example for this app's main concern
│   │   ├── api/                  # Data fetching layer ONLY
│   │   ├── components/           # Feature-specific UI
│   │   ├── hooks/                # useDataRetrieval, useStreamingQuery, etc.
│   │   ├── store/                # Minimal Zustand store (if needed)
│   │   ├── lib/                  # Feature utils + Zod schemas
│   │   └── index.ts              # Barrel export (public API only)
│   └── another-feature/
├── shared/                       # Cross-feature shared code
│   ├── ui/                       # shadcn/ui components ONLY (button, card, etc.)
│   ├── components/               # Composite widgets (use features/ when possible)
│   ├── lib/                      # Pure utilities, date helpers, etc.
│   ├── api/                      # Shared API client + types from packages/types/
│   └── config/                   # Environment, constants
├── entities/                     # Domain models (if any)
├── widgets/                      # Large composite blocks (rare)
└── processes/                    # Complex multi-feature flows (very rare)
```

**Zero Tolerance Rules:**
- **NO** files directly in `src/` root (except `app/`)
- **NO** cross-feature imports: `features/dashboard` cannot import from `features/users`
- **NO** shared code inside features
- **NO** `components/` folder at `src/` level — everything UI goes through `shared/ui/` (shadcn) or feature `components/`
- Folders must be **kebab-case**, components **PascalCase**, hooks **use* camelCase**
- Public exports only via `index.ts` (barrel files)

**Violation = Critical Issue**

---

## 3. Technology Stack — Exact Versions & Patterns (NO DEVIATIONS)

| Technology              | Version / Rule                              | Enforcement |
|-------------------------|---------------------------------------------|-----------|
| Next.js                 | 16+ (App Router)                            | Server Components default |
| React                   | 19 + Compiler                               | No class components |
| TypeScript              | Strict mode (`"strict": true`)              | No `any`, full inference |
| Tailwind CSS            | v4+                                         | Only via `@tailwind` + shadcn |
| shadcn/ui + Radix       | Latest (monorepo setup)                     | **Mandatory** for all new UI |
| Framer Motion           | Latest                                      | Only for complex animations |
| Vercel AI SDK           | v6+                                         | **Primary** for all API/streaming data |
| Biome                   | Latest                                      | Lint + format only |
| Bun                     | Latest                                      | All scripts |
| Zod                     | Latest                                      | All API response validation |
| TanStack Query (optional) | Only if justified                          | Prefer native `fetch` + Server Components |

**Data Retrieval Specific (This App's Core):**
- Use **Vercel AI SDK** `streamText` / `streamObject` / `useChat` for OpenAI-compatible API (Rust backend)
- Server Components: `async function Page() { const data = await fetch(...) }`
- Client data: `useSWR` or `useQuery` **only** when interactive
- **Never** use `useEffect` + `fetch` for new data fetching code
- All responses **must** be validated with Zod schema from `packages/types/`
- Streaming UI: Use `useStreamableValue` or AI SDK hooks with proper loading/error states

---

## 4. Code Standards — Non-Negotiable

### General
- Functional components only
- Default export for page components, named exports for everything else
- One feature = one responsibility (SRP)
- YAGNI: If code is not used today, it does not exist
- KISS: Prefer 10 lines of clear code over 3 lines of clever abstraction

### API Data Retrieval Layer (Critical Focus)
- All API calls go through `features/*/api/` folder
- Types come exclusively from `packages/types/`
- Error handling: Use `error.tsx` + toast (shadcn) + proper HTTP status mapping
- Loading: Use `loading.tsx` + Suspense boundaries
- Caching: Explicit `cache: 'force-cache' | 'no-store' | revalidate`
- Security: Never expose backend API keys in client bundle. Use server actions or Route Handlers
- Rate limiting / auth: Respect backend Tower + Governor middleware (pass API key via headers in server code only)

### UI & Components
- **Every** new UI component must be built on top of shadcn/ui + Radix primitives
- Use `cn()` utility from `shared/lib/utils.ts` for class merging
- No inline styles, no `style` prop except for dynamic values
- All interactive elements must be keyboard accessible (Radix guarantees this)

### State Management
- Prefer React Server State (`searchParams`, `useOptimistic`, form actions)
- Minimal client state → Zustand per feature (never global unless proven necessary)
- No Redux, no MobX, no Jotai unless explicitly approved in architecture decision

### Performance
- Next Image for all images
- Dynamic imports for heavy components
- Proper `revalidatePath` / `revalidateTag` after mutations
- Avoid unnecessary client components (review every `'use client'` directive)

---

## 5. Anti-Patterns — Flag Immediately & Block Merge

1. Cross-feature imports
2. Global pollution (`useContext` at layout level for everything)
3. `useEffect` for data fetching
4. Hardcoded API URLs or secrets in client code
5. Magic strings/numbers
6. `any` type or `@ts-ignore`
7. Custom CSS files (use Tailwind + CSS variables only)
8. Over-abstraction / speculative "future-proof" code
9. Ignoring Biome errors
10. Not using shadcn/ui for buttons, dialogs, tables, etc.
11. Direct `fetch` in client components without proper error/loading states
12. Missing Zod validation on API responses
13. No tests for new data retrieval flows

---

## 6. Review Process (High-Effort Workflow)

**Step 1: Plan Mode (XML format)**
```xml
<review-plan>
  <scope>features/api-data-retrieval + shared/api</scope>
  <critical-issues>
    <issue file="features/api-data-retrieval/api/client.ts" line="42">Cross-feature import from features/users</issue>
  </critical-issues>
  <high-issues>...</high-issues>
  <suggestions count="3">...</suggestions>
</review-plan>
```

**Step 2: Detailed Findings**
- File path + line number for every issue
- Severity: Critical | High | Medium | Low
- Exact code snippet (before/after)
- Reference to this instructions document section

**Step 3: Verification Commands (AI must include)**
```bash
cd apps/web
bun run lint
bun run build
bun test
tsc --noEmit
```

**Step 4: Output Format**
1. Executive Summary (1 paragraph)
2. Severity Table
3. Detailed Findings (grouped by file)
4. Refactored Code Examples (minimal, KISS)
5. Positive Observations
6. Final Verdict: **Approve** | **Request Changes** | **Major Refactor Required**

---

## 7. Final Checklist (AI Must Confirm Every Item)

- [ ] File structure is pure FSD with zero violations
- [ ] All new UI uses shadcn/ui + Radix
- [ ] Server Components used by default
- [ ] API data retrieval uses Vercel AI SDK + Zod + proper states
- [ ] No cross-feature imports
- [ ] Biome passes with zero errors
- [ ] TypeScript strict — zero `any`
- [ ] No `useEffect` data fetching
- [ ] All types come from `packages/types/`
- [ ] Security: API keys only in server code
- [ ] Performance: proper caching, images, dynamic imports
- [ ] Accessibility: full keyboard + ARIA (Radix)
- [ ] Tests exist for data retrieval paths
- [ ] YAGNI / KISS respected (no speculative code)

---

**Enforcement Note:**  
These instructions are derived from community best practices (FSD, shadcn/ui, Next.js official, Vercel AI SDK, Turborepo) and the project's own `CONTRIBUTING.md` / style guide.  

**Any review that does not strictly enforce the above is considered invalid.**

**End of Instructions**