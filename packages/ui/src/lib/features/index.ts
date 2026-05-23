// Moved to ./components/ in Phase 0.1 (2026-05-23); re-exported here for source-compat
// during the strangler-fig migration. Will be renamed to canonical `Composer` in Phase 3.5.
export { default as AgentChatComposer } from '../components/AgentChatComposer.svelte';
export { default as AgentChatStream } from './AgentChatStream.svelte';
export { default as DrawerRecentChats } from './DrawerRecentChats.svelte';
export { default as ToolCallCard } from './ToolCallCard.svelte';
export { default as WorkspaceExplorer } from './WorkspaceExplorer.svelte';
export { default as SuggestionChips } from './SuggestionChips.svelte';
export { default as ContextChip } from './ContextChip.svelte';
export { default as CapabilityRow } from './CapabilityRow.svelte';
export { default as CapabilityBrowser } from './CapabilityBrowser.svelte';
export type { CapEntry } from './CapabilityBrowser.svelte';
export { default as CapabilityPinChip } from './CapabilityPinChip.svelte';

// ── Chrome (top bar, drawer/sidebar, bottom sheet) ──────────────────────────
// Moved from ./chrome/ → ../components/ in Phase 0.1 (2026-05-23).
// Renamed to canonical AppHeader / Drawer / Sheet in Phase 3 per Principle #13.
export { default as AppTopBar } from '../components/AppTopBar.svelte';
export { default as AppDrawer } from '../components/AppDrawer.svelte';
export { default as AppBottomSheet } from '../components/AppBottomSheet.svelte';

// ── Screens (top-level views) ───────────────────────────────────────────────
export { default as ChatScreen } from './screens/ChatScreen.svelte';
export { default as CapabilitiesScreen } from './screens/CapabilitiesScreen.svelte';
export { default as CapabilityDetailSheet } from './screens/CapabilityDetailSheet.svelte';
export { default as ArtifactsScreen } from './screens/ArtifactsScreen.svelte';
export { default as ArtifactRow } from './screens/ArtifactRow.svelte';
export { buildInvocationPrompt } from './screens/buildInvocationPrompt.js';

export { createChatStream } from './createChatStream.svelte.js';
export type { ChatMessage, ToolCardEntry } from './AgentChatStream.svelte';
export type { Attachment } from '../components/AgentChatComposer.svelte';

// ── Routing ─────────────────────────────────────────────────────────────────
export { initialRoute } from '../routing/initialRoute.js';
export type { InitialRoute } from '../routing/initialRoute.js';
export { applyInitialRoute } from '../routing/applyInitialRoute.js';
export type { ApplyInitialRouteHandlers } from '../routing/applyInitialRoute.js';

// ── Live resources ───────────────────────────────────────────────────────────
export { createLiveResource } from '../live/createLiveResource.svelte.js';
export type { LiveResource } from '../live/createLiveResource.svelte.js';
