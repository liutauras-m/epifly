// App shell state
export { createAppShellState } from "./app-shell/app-shell-state.svelte.js";
export type { SidebarWorkspaceNode, CreateAppShellStateArgs } from "./app-shell/app-shell-state.svelte.js";

// Workspace adapters
export { toSidebarWorkspaceNode } from "./workspaces/workspace-adapters.js";

// Workspace node context (layout → page)
export { setWorkspaceNodeContext, getWorkspaceNodeContext } from "./workspaces/workspace-context.svelte.js";
// Active thread node context — breadcrumb + context indicator (Steps 1.4/1.5)
export {
  setActiveThreadNodeContext,
  getActiveThreadNodeContext,
} from "./workspaces/workspace-context.svelte.js";
export type { ActiveThreadNodeContext } from "./workspaces/workspace-context.svelte.js";
// Workspace peek store — "View as document" (Phase 4.1)
export { createPeekStore } from "./workspaces/workspace-peek.store.svelte.js";
export type { PeekStore, PeekRelatedItem } from "./workspaces/workspace-peek.store.svelte.js";
// Workspace actions context — chat pages notify tree of optimistic events (Step 7.1)
export {
  setWorkspaceActionsContext,
  getWorkspaceActionsContext,
} from "./workspaces/workspace-context.svelte.js";
export type { WorkspaceActionsContext, FilingHint } from "./workspaces/workspace-context.svelte.js";

// SDK provider
export { default as SdkProvider } from "./sdk/sdk-provider.svelte";
export { getSdkContext, setSdkContext } from "./sdk/sdk-context.svelte.js";
export {
  clearWebAccessToken,
  createNativeTokenProvider,
  createWebTokenProvider,
  setWebAccessToken,
} from "./sdk/token-provider.js";

// Chat
export type { UiMessage, UiTextMessage, UiStreamEvent, StreamEventKind } from "./chat/chat.types.js";
export { createChatStore } from "./chat/chat.store.svelte.js";
export type { ChatActivityStatus, ChatStoreOptions } from "./chat/chat.store.svelte.js";
export { loadThreadMessages } from "./chat/chat.actions.js";
export { previewContent, isAssistant } from "./chat/chat.utils.js";

// Threads
export { createThreadsStore } from "./threads/threads.store.svelte.js";
export { sortByRecent, threadTitle } from "./threads/threads.utils.js";

// Workspaces
export { createWorkspacesStore } from "./workspaces/workspaces.store.svelte.js";
export { createSmartViewsStore } from "./workspaces/smart-views.store.svelte.js";
export type { SmartViewKind } from "./workspaces/smart-views.store.svelte.js";
export { childrenOf, nodePath } from "./workspaces/workspaces.utils.js";

// Capabilities
export { createCapabilitiesStore } from "./capabilities/capabilities.store.svelte.js";
export { filterCapabilities } from "./capabilities/capabilities.utils.js";

// Files
export {
  uploadWorkspaceFile,
  uploadUiAttachment,
  uploadPersistentFile,
  extractInvoice
} from "./files/files.actions.js";

// Realtime
export { createRealtimeStore } from "./realtime/realtime.store.svelte.js";
export type { RealtimeMessage } from "./realtime/realtime.store.svelte.js";
