# syntax=docker/dockerfile:1.7
# Multi-stage build for the SvelteKit web app (adapter-node).
# Built from the monorepo root so pnpm workspaces resolve correctly.

# ── deps ─────────────────────────────────────────────────────────────────────
FROM node:22-slim AS deps
ENV PNPM_HOME=/root/.local/share/pnpm \
    PATH=/root/.local/share/pnpm:$PATH \
    NODE_ENV=production
RUN corepack enable && corepack prepare pnpm@10.13.1 --activate
WORKDIR /repo

# Copy lockfile + manifests only — maximises Docker layer cache.
COPY pnpm-lock.yaml pnpm-workspace.yaml package.json ./
COPY apps/web/package.json apps/web/
COPY apps/browser-shell/package.json apps/browser-shell/
COPY packages/ui/package.json packages/ui/
COPY packages/sdk/package.json packages/sdk/
COPY packages/types/package.json packages/types/

RUN --mount=type=cache,id=pnpm,target=/root/.local/share/pnpm/store \
    pnpm install --filter web... --filter "./packages/*"

# ── build ────────────────────────────────────────────────────────────────────
FROM deps AS build
ENV NODE_ENV=production
WORKDIR /repo

COPY packages/ packages/
COPY apps/web/ apps/web/

# Build shared packages first, then the web app
RUN pnpm --filter "./packages/*" run build || true
RUN pnpm --filter web build

# Prune to production-only deps for the runtime stage
RUN --mount=type=cache,id=pnpm,target=/root/.local/share/pnpm/store \
    pnpm --filter web deploy --prod /out

# ── runtime ──────────────────────────────────────────────────────────────────
FROM node:22-slim AS runtime
ENV NODE_ENV=production \
    HOST=0.0.0.0 \
    PORT=3000
WORKDIR /app

# Drop root
RUN groupadd --system --gid 1001 nodejs \
 && useradd --system --uid 1001 --gid nodejs --create-home --shell /sbin/nologin nodejs

# adapter-node output + minimal node_modules
COPY --from=build --chown=nodejs:nodejs /repo/apps/web/build ./build
COPY --from=build --chown=nodejs:nodejs /out/node_modules ./node_modules
COPY --from=build --chown=nodejs:nodejs /out/package.json ./package.json

USER nodejs
EXPOSE 3000

HEALTHCHECK --interval=15s --timeout=5s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:3000/').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"

CMD ["node", "build"]
