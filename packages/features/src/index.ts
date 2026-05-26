// Feature stores and actions (rune-based) live in subfolders:
//   sdk/         - SDK provider + context (sdk-context.svelte.ts, sdk-provider.svelte)
//   chat/        - chat.store.svelte.ts wrapping sdk.chat.stream
//   threads/     - threads.store.svelte.ts wrapping sdk.threads.*
//   workspaces/  - workspaces.store.svelte.ts wrapping sdk.workspaces.*
//   files/       - files.actions.ts naming intent for each upload endpoint
// See docs/plan.md for the full feature architecture.
export {};
