// ── Billing components ────────────────────────────────────────────────────────
export { default as PlanBadge } from "./components/PlanBadge.svelte";
export { default as PlanCard } from "./components/PlanCard.svelte";
export { default as UsageMeter } from "./components/UsageMeter.svelte";
export { default as QuotaBanner } from "./components/QuotaBanner.svelte";

// ── Primitive components ─────────────────────────────────────────────────────
export { default as Type } from "./components/Type.svelte";
export type { TypeVariant } from "./components/Type.svelte";
export { default as Icon } from "./components/Icon.svelte";
export type { IconSize } from "./components/Icon.svelte";
export { default as Button } from "./components/Button.svelte";
export type { ButtonVariant, ButtonSize } from "./components/Button.svelte";
export { default as Field } from "./components/Field.svelte";
export type { FieldType } from "./components/Field.svelte";
export { default as Chip } from "./components/Chip.svelte";
export type { ChipVariant, ChipSize } from "./components/Chip.svelte";
export { default as EmptyState } from "./components/EmptyState.svelte";
export type { EmptyStateKind } from "./components/EmptyState.svelte";
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
export { default as AgentChatComposer } from "./components/AgentChatComposer.svelte"; // moved in Phase 0.1 (2026-05-23); will be renamed to `Composer` in Phase 3.5
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
// Moved from ./features/chrome/ → ./components/ in Phase 0.1 (2026-05-23).
// These names will be renamed to canonical AppHeader / Drawer / Sheet in Phase 3 (Principle #13);
// the App* names here remain as the strangler-fig shim until Phase 4 close.
export { default as AppTopBar } from "./components/AppTopBar.svelte";
export { default as AppDrawer } from "./components/AppDrawer.svelte";
export { default as AppBottomSheet } from "./components/AppBottomSheet.svelte";

// ── Screens ─────────────────────────────────────────────────────────────────
export { default as ChatScreen } from "./features/screens/ChatScreen.svelte";
export { default as CapabilitiesScreen } from "./features/screens/CapabilitiesScreen.svelte";
export { default as CapabilityDetailSheet } from "./features/screens/CapabilityDetailSheet.svelte";
export { default as ArtifactsScreen } from "./features/screens/ArtifactsScreen.svelte";
export { default as ArtifactRow } from "./features/screens/ArtifactRow.svelte";
export { buildInvocationPrompt } from "./features/screens/buildInvocationPrompt.js";
export type { ChatMessage, ToolCardEntry } from "./features/AgentChatStream.svelte";
export type { Attachment } from "./components/AgentChatComposer.svelte"; // moved in Phase 0.1

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
