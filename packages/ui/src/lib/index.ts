// ── Billing components ────────────────────────────────────────────────────────
export { default as PlanBadge } from "./components/PlanBadge.svelte";
export { default as PlanCard } from "./components/PlanCard.svelte";
export { default as UsageMeter } from "./components/UsageMeter.svelte";
export { default as QuotaBanner } from "./components/QuotaBanner.svelte";

// ── Primitive components ─────────────────────────────────────────────────────
export { default as AppShell } from "./components/AppShell.svelte";
export { default as CapabilityCard } from "./components/CapabilityCard.svelte";
export { default as ToastHost } from "./components/ToastHost.svelte";
export { default as WorkspaceTree } from "./components/WorkspaceTree.svelte";
export type { Toast } from "./components/ToastHost.svelte";

// ── Theme system ─────────────────────────────────────────────────────────────
export { default as ThemeProvider } from "./components/ThemeProvider.svelte";
export { default as ThemeSwitcher } from "./components/ThemeSwitcher.svelte";
export { THEME_SCRIPT } from "./components/ThemeScript.js";

// ── Features ─────────────────────────────────────────────────────────────────
export { default as AgentChatComposer } from "./features/AgentChatComposer.svelte";
export { default as AgentChatStream } from "./features/AgentChatStream.svelte";
export { default as HostedProjectCard } from "./features/HostedProjectCard.svelte";
export { default as ToolCallCard } from "./features/ToolCallCard.svelte";
export { default as WorkspaceExplorer } from "./features/WorkspaceExplorer.svelte";
export { default as SuggestionChips } from "./features/SuggestionChips.svelte";
export { default as ContextChip } from "./features/ContextChip.svelte";
export { default as CapabilityRow } from "./features/CapabilityRow.svelte";
export { default as CapabilityBrowser } from "./features/CapabilityBrowser.svelte";
export type { CapEntry } from "./features/CapabilityBrowser.svelte";

// ── Chrome ──────────────────────────────────────────────────────────────────
export { default as AppTopBar } from "./features/chrome/AppTopBar.svelte";
export { default as AppDrawer } from "./features/chrome/AppDrawer.svelte";
export { default as AppBottomSheet } from "./features/chrome/AppBottomSheet.svelte";

// ── Screens ─────────────────────────────────────────────────────────────────
export { default as ChatScreen } from "./features/screens/ChatScreen.svelte";
export { default as CapabilitiesScreen } from "./features/screens/CapabilitiesScreen.svelte";
export { default as CapabilityDetailSheet } from "./features/screens/CapabilityDetailSheet.svelte";
export { default as ArtifactsScreen } from "./features/screens/ArtifactsScreen.svelte";
export { default as ArtifactRow } from "./features/screens/ArtifactRow.svelte";
export { buildInvocationPrompt } from "./features/screens/buildInvocationPrompt.js";
export type { ChatMessage, ToolCardEntry } from "./features/AgentChatStream.svelte";
export type { Attachment } from "./features/AgentChatComposer.svelte";

// ── Utils ────────────────────────────────────────────────────────────────────
export { default as LiveAnnouncer } from "./utils/LiveAnnouncer.svelte";
export { autoGrow } from "./utils/actions.js";
export { prefersReducedMotion } from "./utils/motion-prefs.js";

// ── Motion primitives ────────────────────────────────────────────────────────
export {
  springAnimate,
  recordRect,
  playFlip,
  stagger,
  tap,
  startViewTransition,
} from "./motion/index.js";
export type { SpringOpts } from "./motion/index.js";

// ── Stores ───────────────────────────────────────────────────────────────────
export { createThemeStore, localStorageAdapter } from "./stores/themeStore.svelte.js";
export type { Theme, ThemeAdapter, ThemeStore } from "./stores/themeStore.svelte.js";
export { createFeatureFlags } from "./stores/featureFlags.svelte.js";
export type { FeatureFlags, FeatureFlagStore } from "./stores/featureFlags.svelte.js";
export { toasts } from "./stores/toast.svelte.js";
export type { ToastKind } from "./stores/toast.svelte.js";
export { modeStore } from "./stores/modeStore.svelte.js";
export type { AppMode } from "./stores/modeStore.svelte.js";
export { recentsStore } from "./stores/recents.svelte.js";
export { breadcrumbsStore } from "./stores/breadcrumbs.svelte.js";

// ── Routing ──────────────────────────────────────────────────────────────────
export { initialRoute } from "./routing/initialRoute.js";
export type { InitialRoute } from "./routing/initialRoute.js";
export { applyInitialRoute } from "./routing/applyInitialRoute.js";
export type { ApplyInitialRouteHandlers } from "./routing/applyInitialRoute.js";

// ── Capabilities ─────────────────────────────────────────────────────────────
export { createCapabilityRendererRegistry } from "./capabilities/CapabilityRendererRegistry.js";
export type { CapabilityRendererRegistry, CreateRegistryOpts } from "./capabilities/CapabilityRendererRegistry.js";
export { provideCapabilityRendererRegistry, useCapabilityRendererRegistry } from "./capabilities/CapabilityRendererRegistry.svelte.js";
