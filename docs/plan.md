Below is the implementation brief I’d give to an AI coding agent. It is adjusted for the uploaded SDK files and for a shared **Web + Tauri v2 desktop/mobile** product. The goal is not “make folders.” The goal is to prevent the app from becoming a cross-platform lasagna.

---

# AI implementation instructions

Build a Svelte 5 + SvelteKit app using shadcn-svelte UI components and the provided Conus SDK. The product must support:

```txt
Web
iOS
Android
macOS
Windows
```

Use a monorepo with separate runtime apps:

```txt
apps/web       = SvelteKit web app, can use SSR/server routes
apps/native    = SvelteKit SPA inside Tauri v2
packages/sdk   = uploaded Conus SDK
packages/ui    = shared Svelte UI components
packages/shared = shared types, schemas, utilities
```

Do **not** build one runtime full of `if (tauri)` hacks. That is not architecture; that is denial with syntax highlighting.

---

# Required documentation sources

Before implementation, read these docs and follow them as source of truth:

```txt
Svelte LLM docs
https://svelte.dev/docs/llms

Svelte full LLM docs
https://svelte.dev/llms-full.txt

SvelteKit LLM docs
https://svelte.dev/docs/kit/llms.txt

Tauri v2 docs
https://v2.tauri.app/

Tauri + SvelteKit guide
https://v2.tauri.app/start/frontend/sveltekit/

Tauri capabilities / permissions
https://v2.tauri.app/security/capabilities/
https://v2.tauri.app/security/permissions/

shadcn-svelte components
https://www.shadcn-svelte.com/docs/components

shadcn-svelte Sidebar
https://www.shadcn-svelte.com/docs/components/sidebar
```

Svelte officially exposes `/llms.txt`, `/llms-full.txt`, `/llms-medium.txt`, and package-level LLM docs for Svelte, SvelteKit, and CLI, so use those instead of guessing from stale examples. ([Svelte][1])

For Tauri + SvelteKit, use `adapter-static`, SPA mode, and `build/` as `frontendDist`; Tauri does not support server-based SvelteKit solutions inside the app shell. ([Tauri][2])

For shadcn-svelte, use the official component registry and keep components composable; the Sidebar docs explicitly say the sidebar files are starting points and can be customized as app-owned code. ([shadcn-svelte.com][3])

---

# Uploaded SDK assessment

The uploaded SDK is already shaped as a client package. Keep it as a separate package, not scattered into the app.

Current SDK modules include:

```txt
auth.ts
capabilities.ts
chat.ts
chatApi.ts
client.ts
endpoints.ts
files.ts
glyphs.ts
index.ts
realtime.ts
shells.ts
threads.ts
types.ts
ui.ts
workspaces.ts
```

The central factory is `createConusSdk(opts)`, which wires `auth`, `capabilities`, `chat`, `files`, `threads`, `ui`, `workspaces`, `realtime`, and `shells` around a shared internal client. 

The SDK already has a useful `ApiResult<T>` pattern:

```ts
type ApiResult<T> =
  | { data: T; error: null }
  | { data: null; error: ApiError };
```

Use it consistently in the app. Do not throw errors from UI-facing feature stores unless the SDK method itself throws. The SDK’s `call` helper already converts failures into `{ data: null, error }`. 

The chat streaming module exposes an async generator that yields typed deltas such as `text`, `tool_start`, `tool_result`, `routing_meta`, `resource_invalidated`, `thread_id`, and `done`. Build the UI around that event model instead of inventing a second stream protocol.  

The SDK endpoint map is already centralized in `EP`. Do not hardcode endpoint strings in UI or feature code. 

---

# Final monorepo structure

```txt
repo/
  apps/
    web/
      src/
        app.css
        app.d.ts
        hooks.server.ts

        routes/
          +layout.svelte

          (auth)/
            login/
              +page.svelte

          (app)/
            +layout.svelte
            +page.svelte

            chat/
              +page.svelte
              [threadId]/
                +page.svelte

            workspaces/
              +page.svelte

            settings/
              +page.svelte

        lib/
          server/
            auth/
            api/
            session/

      svelte.config.js
      vite.config.ts
      package.json

    native/
      src/
        app.css
        app.d.ts

        routes/
          +layout.svelte
          +layout.ts

          (app)/
            +layout.svelte
            +page.svelte

            chat/
              +page.svelte
              [threadId]/
                +page.svelte

            workspaces/
              +page.svelte

            settings/
              +page.svelte

        lib/
          native/
            platform.ts
            token-provider.ts
            safe-area.ts
            window.ts

      src-tauri/
        tauri.conf.json
        capabilities/
          default.json
          desktop.json
          mobile.json
        src/
          main.rs
          lib.rs
        icons/

      svelte.config.js
      vite.config.ts
      package.json

  packages/
    sdk/
      src/
        auth.ts
        capabilities.ts
        chat.ts
        chatApi.ts
        client.ts
        endpoints.ts
        files.ts
        glyphs.ts
        index.ts
        realtime.ts
        shells.ts
        threads.ts
        types.ts
        ui.ts
        workspaces.ts
      package.json
      tsconfig.json

    ui/
      src/
        components/
          ui/
            button/
            textarea/
            sidebar/
            dropdown-menu/
            avatar/
            separator/
            scroll-area/
            tooltip/
            sheet/
            skeleton/
            sonner/

          app/
            app-shell.svelte
            app-sidebar.svelte
            app-mobile-header.svelte
            app-main.svelte
            app-safe-area.svelte

          chat/
            chat-composer.svelte
            chat-empty-state.svelte
            chat-thread.svelte
            chat-message.svelte
            chat-message-list.svelte
            chat-stream-status.svelte
            tool-event-row.svelte
            routing-meta-row.svelte

          workspace/
            workspace-tree.svelte
            workspace-switcher.svelte
            workspace-node-row.svelte

          account/
            account-menu.svelte
            user-avatar.svelte

        styles/
          tokens.css
          motion.css

        utils/
          cn.ts

        index.ts

    features/
      src/
        sdk/
          sdk-context.svelte.ts
          sdk-provider.svelte
          token-provider.ts

        chat/
          chat.types.ts
          chat.store.svelte.ts
          chat.actions.ts
          chat.utils.ts

        threads/
          threads.store.svelte.ts
          threads.utils.ts

        workspaces/
          workspaces.store.svelte.ts
          workspaces.utils.ts

        capabilities/
          capabilities.store.svelte.ts
          capabilities.utils.ts

        files/
          files.actions.ts

    shared/
      src/
        constants/
        platform/
        types/
        utils/

  package.json
  pnpm-workspace.yaml
  turbo.json
  tsconfig.base.json
```

---

# Naming conventions

Use kebab-case filenames:

```txt
app-shell.svelte
chat-composer.svelte
threads.store.svelte.ts
sdk-context.svelte.ts
token-provider.ts
```

Use PascalCase imports:

```ts
import AppShell from "@conusai/ui/components/app/app-shell.svelte";
import ChatComposer from "@conusai/ui/components/chat/chat-composer.svelte";
```

Use `.svelte.ts` only when using Svelte runes:

```txt
chat.store.svelte.ts
threads.store.svelte.ts
sdk-context.svelte.ts
```

Use plain `.ts` for non-rune utilities:

```txt
chat.utils.ts
token-provider.ts
platform.ts
```

Do not name folders `layout`. SvelteKit already has `+layout.svelte`; calling a component folder `layout` just creates fog. Use:

```txt
components/app/
```

---

# Package setup

## `packages/sdk`

Move the uploaded files into:

```txt
packages/sdk/src/
```

Add package exports:

```json
{
  "name": "@conusai/sdk",
  "type": "module",
  "exports": {
    ".": "./src/index.ts"
  }
}
```

The SDK’s `index.ts` already exports `createConusSdk`, `ConusSdk`, `TokenProvider`, `ClientOpts`, `streamChat`, endpoint constants, and core types. Keep that public API. 

## `packages/ui`

shadcn-svelte components live here:

```txt
packages/ui/src/components/ui/
```

Product components live outside `ui`:

```txt
packages/ui/src/components/app/
packages/ui/src/components/chat/
packages/ui/src/components/workspace/
packages/ui/src/components/account/
```

Do not put `chat-composer.svelte` into `components/ui`. It is not a primitive. It is product UI.

---

# Svelte 5 coding rules

Use Svelte 5 runes and modern event syntax.

## Props

Use `$props()`:

```svelte
<script lang="ts">
  type Props = {
    disabled?: boolean;
    placeholder?: string;
    onSubmit?: (value: string) => void | Promise<void>;
  };

  let {
    disabled = false,
    placeholder = "Message...",
    onSubmit
  }: Props = $props();
</script>
```

Do not use old `export let` in new components.

## Events

Use modern event attributes:

```svelte
<form onsubmit={handleSubmit}>
<button onclick={handleClick}>
```

Do not use old `on:click` unless a specific dependency requires it.

## Derived state

Use `$derived`:

```ts
let canSend = $derived(message.trim().length > 0 && !isStreaming);
```

Do not use `$effect` for derived state. That is how clean code becomes plumbing with opinions.

## Effects

Use `$effect` only for:

```txt
focus management
scroll-to-bottom
external subscriptions
analytics
DOM measurement
native bridge setup
```

---

# SDK integration pattern

Create a runtime-neutral SDK provider in `packages/features`.

```txt
packages/features/src/sdk/
  sdk-context.svelte.ts
  sdk-provider.svelte
  token-provider.ts
```

## `sdk-context.svelte.ts`

```ts
import { getContext, setContext } from "svelte";
import type { ConusSdk } from "@conusai/sdk";

const SDK_CONTEXT = Symbol("conus-sdk");

export function setSdkContext(sdk: ConusSdk) {
  setContext(SDK_CONTEXT, sdk);
}

export function getSdkContext(): ConusSdk {
  const sdk = getContext<ConusSdk | undefined>(SDK_CONTEXT);
  if (!sdk) throw new Error("Conus SDK context is missing");
  return sdk;
}
```

## `sdk-provider.svelte`

```svelte
<script lang="ts">
  import { createConusSdk, type TokenProvider } from "@conusai/sdk";
  import { setSdkContext } from "./sdk-context.svelte";

  type Props = {
    baseUrl: string;
    tokenProvider: TokenProvider;
    children?: import("svelte").Snippet;
  };

  let { baseUrl, tokenProvider, children }: Props = $props();

  const sdk = createConusSdk({
    baseUrl,
    tokenProvider,
    fetch: globalThis.fetch.bind(globalThis)
  });

  setSdkContext(sdk);
</script>

{@render children?.()}
```

Reason: the uploaded SDK factory expects `fetch`, `baseUrl`, and `tokenProvider`. 

---

# Token provider rules

The SDK expects:

```ts
interface TokenProvider {
  get(): Promise<string | null>;
}
```

That is already defined in the SDK. 

Use separate token providers per runtime.

## Web token provider

For web, prefer server-managed auth/cookies where possible. If the browser app must call the API directly, expose a safe token retrieval method. Do not blindly store long-lived tokens in `localStorage`.

```ts
export function createWebTokenProvider(): TokenProvider {
  return {
    async get() {
      return null;
    }
  };
}
```

Adjust according to actual auth model.

## Native token provider

For Tauri, store tokens through a native-safe storage plugin or scoped app storage. Do not use random localStorage as the final implementation.

```ts
export function createNativeTokenProvider(): TokenProvider {
  return {
    async get() {
      // TODO: read from secure/native storage abstraction
      return null;
    }
  };
}
```

---

# Chat implementation

The SDK already provides:

```ts
sdk.chat.stream(params, opts)
```

`chatApi.ts` wraps `streamChat` and injects `baseUrl` and `fetch` from the internal client. 

Use it directly in a rune store.

## `chat.store.svelte.ts`

```ts
import type { ConusSdk, ChatStreamDelta } from "@conusai/sdk";

export type UiMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  pending?: boolean;
};

export function createChatStore(sdk: ConusSdk) {
  let messages = $state<UiMessage[]>([]);
  let isStreaming = $state(false);
  let threadId = $state<string | null>(null);
  let error = $state<string | null>(null);
  let abortController = $state<AbortController | null>(null);

  async function send(message: string, workspaceNodeId?: string | null) {
    const trimmed = message.trim();
    if (!trimmed || isStreaming) return;

    error = null;
    isStreaming = true;

    const controller = new AbortController();
    abortController = controller;

    messages.push({
      id: crypto.randomUUID(),
      role: "user",
      content: trimmed
    });

    const assistantMessage: UiMessage = {
      id: crypto.randomUUID(),
      role: "assistant",
      content: "",
      pending: true
    };

    messages.push(assistantMessage);

    try {
      for await (const delta of sdk.chat.stream({
        message: trimmed,
        threadId,
        workspaceNodeId,
        signal: controller.signal
      })) {
        applyDelta(delta, assistantMessage);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      assistantMessage.pending = false;
    } finally {
      isStreaming = false;
      abortController = null;
      assistantMessage.pending = false;
    }
  }

  function applyDelta(delta: ChatStreamDelta, assistantMessage: UiMessage) {
    switch (delta.kind) {
      case "text":
        assistantMessage.content += delta.content;
        break;

      case "thread_id":
        threadId = delta.id;
        break;

      case "tool_start":
      case "tool_result":
      case "routing_meta":
      case "resource_invalidated":
        // TODO: render as structured stream events in chat-stream-status.svelte
        break;

      case "done":
        assistantMessage.pending = false;
        break;
    }
  }

  function stop() {
    abortController?.abort();
  }

  return {
    get messages() {
      return messages;
    },
    get isStreaming() {
      return isStreaming;
    },
    get threadId() {
      return threadId;
    },
    get error() {
      return error;
    },
    send,
    stop
  };
}
```

Do not parse raw SSE in UI components. `chat.ts` already does parsing, backoff, tool events, routing metadata, invalidation events, and thread ID extraction. 

---

# Threads implementation

Use `sdk.threads.list()` and `sdk.threads.messages(threadId)`.

The uploaded `threads.ts` unwraps the backend’s `{ data: [...] }` envelope so UI callers receive arrays directly. 

Create:

```txt
packages/features/src/threads/
  threads.store.svelte.ts
```

Rules:

```txt
load thread summaries for sidebar
load messages when opening /chat/[threadId]
keep route files thin
do not duplicate ThreadSummary or ThreadMessage types
```

The SDK exports `ThreadMessage` and `ThreadSummary`; use those. 

---

# Workspaces implementation

Use `sdk.workspaces`.

Available methods include:

```txt
tree
get
create
search
getContent
patchContent
move
delete
share
unshare
upload
```

These are already in `workspaces.ts`. 

Create:

```txt
packages/features/src/workspaces/
  workspaces.store.svelte.ts
  workspaces.utils.ts
```

Rules:

```txt
workspace tree state belongs in a feature store
workspace node rendering belongs in UI components
workspace API calls must go through SDK
do not hardcode /v1/workspaces paths
```

---

# Files and uploads

There are two upload APIs in the SDK:

```txt
files.upload(file)      -> /v1/files
ui.upload(file)         -> EP.UI_UPLOAD
workspaces.upload(file) -> EP.UI_UPLOAD
```

`files.upload` posts to `/v1/files`. 

`ui.upload` posts to `EP.UI_UPLOAD`, and `ui.extractInvoice(fileId)` posts to `EP.UI_EXTRACT_INVOICE`. 

Do not let components choose randomly between them. Create a single feature action:

```txt
packages/features/src/files/files.actions.ts
```

And explicitly name intent:

```ts
uploadWorkspaceFile(file)
uploadUiAttachment(file)
uploadPersistentFile(file)
extractInvoice(fileId)
```

Otherwise six months from now nobody will know which upload endpoint is the “real” one. Classic little API crime scene.

---

# Realtime and shells

The SDK includes:

```txt
sdk.realtime.subscribe()
sdk.shells.control(deviceId)
sdk.shells.parseMessage(data)
```

`realtime.subscribe()` creates a websocket to `/api/realtime/workspace` and reconnects with exponential backoff and jitter. 

`shells.control(deviceId)` opens a websocket to `/v1/shells/{deviceId}/control`. 

Rules:

```txt
never open websocket connections directly in components
wrap subscriptions inside feature stores
close sockets in $effect cleanup
show connection status in UI
do not enable shell controls in mobile unless explicitly required
```

---

# UI architecture

Use shadcn-svelte for primitives:

```bash
pnpm dlx shadcn-svelte@latest add sidebar button textarea dropdown-menu avatar separator scroll-area tooltip sheet skeleton sonner
```

Minimum app shell:

```txt
AppShell
AppSidebar
AppMobileHeader
ChatComposer
ChatThread
WorkspaceTree
AccountMenu
```

The shadcn-svelte Sidebar must wrap the app with `Sidebar.Provider`, and the sidebar components are meant to be composed with things like DropdownMenu and Dialog. ([shadcn-svelte.com][3])

## Desktop shell

```txt
persistent sidebar
centered composer
chat/thread area
account footer
workspace/thread navigation
```

## Mobile shell

```txt
no permanent sidebar
top-left sidebar trigger
off-canvas sidebar
safe-area-aware composer
large tap targets
```

## Tauri desktop

```txt
same app shell as web desktop
optional custom titlebar later
respect min window size
updater only if configured
```

## Tauri mobile

```txt
safe-area padding
keyboard-aware composer
no hover-only UI
avoid nested scroll traps
back-button behavior on Android
```

---

# Native app requirements

In `apps/native`, configure SvelteKit exactly as a SPA/static app.

## `apps/native/svelte.config.js`

```js
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: "build",
      assets: "build",
      fallback: "index.html"
    })
  }
};

export default config;
```

## `apps/native/src/routes/+layout.ts`

```ts
export const ssr = false;
export const prerender = false;
```

This follows Tauri’s SvelteKit guidance: SPA mode is recommended because `load` functions then run in the webview and can access Tauri APIs; prerender/build-time `load` functions cannot. ([Tauri][2])

## `apps/native/src-tauri/tauri.conf.json`

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ConusAI",
  "version": "0.1.0",
  "identifier": "com.conusai.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../build"
  },
  "app": {
    "windows": [
      {
        "title": "ConusAI",
        "width": 1200,
        "height": 800,
        "minWidth": 360,
        "minHeight": 640,
        "resizable": true
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

Adjust app name and identifier before release.

---

# Tauri security rules

Use minimum capabilities.

```txt
src-tauri/capabilities/default.json
src-tauri/capabilities/desktop.json
src-tauri/capabilities/mobile.json
```

Start with only:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default app permissions",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

Add plugins only when product requirements force it.

Do not enable:

```txt
fs
shell
process
clipboard
http
notification
updater
```

unless there is an implemented feature using it. “Might need later” is not a permission model; it is optimism with liability.

Tauri v2 capabilities define which permissions are granted to windows/webviews, and permissions can allow or deny commands with scopes, so keep these files explicit and small. ([Tauri][2])

---

# Styling rules

Use:

```txt
Tailwind utilities in components
CSS variables for design tokens
motion.css for shared motion timing
shadcn-svelte components for primitives
```

## `packages/ui/src/styles/tokens.css`

```css
:root {
  --app-header-height: 3rem;
  --composer-height: 3.5rem;

  --safe-top: env(safe-area-inset-top);
  --safe-bottom: env(safe-area-inset-bottom);

  --radius-app: 1rem;
}
```

## `packages/ui/src/styles/motion.css`

```css
:root {
  --motion-fast: 120ms;
  --motion-base: 180ms;
  --motion-slow: 240ms;

  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --ease-emphasized: cubic-bezier(0.16, 1, 0.3, 1);
  --ease-exit: cubic-bezier(0.4, 0, 1, 1);
}

@media (prefers-reduced-motion: reduce) {
  * {
    animation-duration: 1ms !important;
    transition-duration: 1ms !important;
    scroll-behavior: auto !important;
  }
}
```

Premium minimal design rule:

```txt
movement under 8px
duration 120–240ms
no animated blobs
no glassmorphism abuse
no “AI glow” unless explicitly designed
focus states must be visible
hover states must not be required on mobile
```

---

# Implementation order

## Phase 1 — repo foundation

```txt
create pnpm workspace
create apps/web
create apps/native
create packages/sdk
create packages/ui
create packages/features
create packages/shared
move uploaded SDK files into packages/sdk/src
wire TypeScript paths
verify package imports
```

## Phase 2 — shadcn-svelte UI foundation

```txt
install shadcn-svelte in packages/ui or app-local strategy
add sidebar, button, textarea, dropdown-menu, avatar, separator, scroll-area, tooltip, sheet, skeleton, sonner
create tokens.css and motion.css
create cn.ts
create app-shell.svelte
create app-sidebar.svelte
create app-mobile-header.svelte
create chat-composer.svelte
```

## Phase 3 — SDK provider

```txt
create sdk-context.svelte.ts
create sdk-provider.svelte
create web token provider
create native token provider placeholder
wrap apps/web and apps/native with provider
```

## Phase 4 — chat

```txt
create chat.store.svelte.ts
use sdk.chat.stream
render text deltas
capture thread_id
render tool/routing/resource events as subtle status rows
implement stop streaming
handle errors
```

## Phase 5 — threads/sidebar

```txt
load sdk.threads.list
render recent threads in sidebar
open /chat/[threadId]
load sdk.threads.messages(threadId)
keep routes thin
```

## Phase 6 — workspaces

```txt
load sdk.workspaces.tree
render workspace tree
wire workspaceNodeId into chat send
support search later
```

## Phase 7 — native hardening

```txt
configure Tauri SPA static build
add minimal capabilities
add platform detection
add safe-area component
test desktop window sizes
test iOS/Android keyboard behavior
```

---

# Hard rules for the AI coding agent

```txt
1. Do not hardcode API paths outside packages/sdk.
2. Do not parse SSE in components; use sdk.chat.stream.
3. Do not put product components inside components/ui.
4. Do not use export let in new Svelte components.
5. Do not use on:click in new Svelte components.
6. Do not use $effect for derived state.
7. Do not put server code in shared packages used by native.
8. Do not import Tauri APIs in shared UI components.
9. Do not enable broad Tauri permissions.
10. Do not create duplicate SDK clients per component.
11. Do not use one SvelteKit config for both web and native.
12. Do not store tokens in localStorage as final native auth.
13. Do not build custom sidebar primitives before using shadcn-svelte Sidebar.
14. Do not create folders for imaginary future features.
```

---

# Final architecture

```txt
apps/web
  SSR-capable SvelteKit web app

apps/native
  static SPA SvelteKit inside Tauri v2

packages/sdk
  uploaded Conus SDK, source of truth for API access

packages/ui
  shadcn-svelte primitives + shared product UI

packages/features
  Svelte rune stores and feature actions using SDK

packages/shared
  runtime-neutral constants, types, utilities
```

This is the clean version: SDK-owned networking, feature-owned state, UI-owned rendering, app-owned runtime configuration. Anything else will slowly become a haunted house with TypeScript.

[1]: https://svelte.dev/docs/llms?utm_source=chatgpt.com "Docs for LLMs"
[2]: https://v2.tauri.app/start/frontend/sveltekit/?utm_source=chatgpt.com "SvelteKit"
[3]: https://shadcn-svelte.com/docs/components/sidebar?utm_source=chatgpt.com "Sidebar"
