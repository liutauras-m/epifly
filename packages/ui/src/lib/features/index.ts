// Phase 4 close: AgentChatComposer/AppTopBar/AppDrawer/AppBottomSheet/WorkspaceExplorer shims deleted.
// Use canonical names: Composer, AppHeader, Drawer, Sheet, WorkspaceTree.
export { default as AgentChatStream } from './AgentChatStream.svelte';
export { default as DrawerRecentChats } from './DrawerRecentChats.svelte';
export { default as ToolCallCard } from './ToolCallCard.svelte';
// Phase 4.7: WorkspaceTree is the canonical name (features/, not components/).
export { default as WorkspaceTree } from './WorkspaceTree.svelte';
export { default as SuggestionChips } from './SuggestionChips.svelte';
export { default as ContextChip } from './ContextChip.svelte';
export { default as CapabilityRow } from './CapabilityRow.svelte';
export { default as CapabilityBrowser } from './CapabilityBrowser.svelte';
export type { CapEntry } from './CapabilityBrowser.svelte';
export { default as CapabilityPinChip } from './CapabilityPinChip.svelte';

// ── Screens (top-level views) ───────────────────────────────────────────────
export { default as ChatScreen } from './screens/ChatScreen.svelte';
export { default as CapabilitiesScreen } from './screens/CapabilitiesScreen.svelte';
export { default as CapabilityDetailSheet } from './screens/CapabilityDetailSheet.svelte';
export { default as ArtifactsScreen } from './screens/ArtifactsScreen.svelte';
export { default as ArtifactRow } from './screens/ArtifactRow.svelte';
export { buildInvocationPrompt } from './screens/buildInvocationPrompt.js';

export { createChatStream } from './createChatStream.svelte.js';
export type { CustomStreamFn } from './createChatStream.svelte.js';
export type { ToolCardEntry } from './AgentChatStream.svelte';
export type { ChatMessage } from '../components/MessageList.svelte';
export type { Attachment } from '../components/Composer.svelte';

// ── Routing ─────────────────────────────────────────────────────────────────
export { initialRoute } from '../routing/initialRoute.js';
export type { InitialRoute } from '../routing/initialRoute.js';
export { applyInitialRoute } from '../routing/applyInitialRoute.js';
export type { ApplyInitialRouteHandlers } from '../routing/applyInitialRoute.js';

// ── Shell composition ────────────────────────────────────────────────────────
// Full authenticated + login screens for browser-shell / Tauri consumers.
export { default as ShellScreen }      from './ShellScreen.svelte';
export { default as ShellLoginScreen } from './ShellLoginScreen.svelte';
// ShellPage = single mount-point: wraps ShellScreen with deep-link restore
// and workspace-URL sync. Both apps' root +page.svelte use this.
export { default as ShellPage }        from './ShellPage.svelte';

// ── Domain features ──────────────────────────────────────────────────────────
export { default as QuotaList } from './QuotaList.svelte';
export type { QuotaItem } from './QuotaList.svelte';
export { default as ProfileSheet } from './ProfileSheet.svelte';
// Phase 3.5: AttachmentSheet migrated from apps/browser-shell/src/lib/mobile/parts/
export { default as AttachmentSheet } from './AttachmentSheet.svelte';

// Billing
export { default as InvoiceStatusBadge } from './billing/InvoiceStatusBadge.svelte';
export type { InvoiceStatus } from './billing/InvoiceStatusBadge.svelte';

// ── Live resources ───────────────────────────────────────────────────────────
export { createLiveResource } from '../live/createLiveResource.svelte.js';
export type { LiveResource } from '../live/createLiveResource.svelte.js';
