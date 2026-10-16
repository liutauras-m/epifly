# syntax=docker/dockerfile:1.7
# SvelteKit web app (adapter-node). Build context: the repository root.

FROM node:24.22.0-trixie-slim AS build
ENV PNPM_HOME=/pnpm PATH=/pnpm:$PATH
RUN corepack enable && corepack prepare pnpm@10.13.1 --activate
WORKDIR /repo
COPY pnpm-lock.yaml pnpm-workspace.yaml package.json ./
COPY apps/web/package.json apps/web/
COPY packages/ui/package.json packages/ui/
COPY packages/sdk/package.json packages/sdk/
COPY packages/types/package.json packages/types/
RUN --mount=type=cache,id=pnpm,target=/pnpm/store \
    pnpm install --frozen-lockfile --filter web...
COPY packages/ packages/
COPY apps/web/ apps/web/
RUN pnpm --filter web... run build
RUN --mount=type=cache,id=pnpm,target=/pnpm/store \
    pnpm --filter web deploy --prod --legacy /out

FROM node:24.22.0-trixie-slim AS runtime
ENV NODE_ENV=production HOST=0.0.0.0 PORT=3000
WORKDIR /app
COPY --from=build --chown=10001:10001 /repo/apps/web/build ./build
COPY --from=build --chown=10001:10001 /out/node_modules ./node_modules
COPY --from=build --chown=10001:10001 /out/package.json ./package.json
USER 10001:10001
EXPOSE 3000
HEALTHCHECK --interval=15s --timeout=5s --retries=5 \
  CMD node -e "fetch('http://127.0.0.1:3000/healthz').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
CMD ["node", "build"]
