// ── Billing components ────────────────────────────────────────────────────────
export { default as PlanBadge } from "./components/PlanBadge.svelte";
export { default as PlanCard } from "./components/PlanCard.svelte";
export { default as UsageMeter } from "./components/UsageMeter.svelte";
export { default as QuotaBanner } from "./components/QuotaBanner.svelte";

// ── Primitive components ─────────────────────────────────────────────────────
export { default as Type } from "./components/Type.svelte";
export type { TypeVariant } from "./components/Type.svelte";
export { default as Icon } from "./components/Icon.svelte";
export type { IconSize, IconComponent } from "./components/Icon.types.js";
export { default as Button } from "./components/Button.svelte";
export type { ButtonVariant, ButtonSize } from "./components/Button.svelte";
export { default as Field } from "./components/Field.svelte";
export type { FieldType } from "./components/Field.svelte";
export { default as Chip } from "./components/Chip.svelte";
export type { ChipVariant, ChipSize } from "./components/Chip.svelte";
export { default as EmptyState } from "./components/EmptyState.svelte";
export type { EmptyStateKind } from "./components/EmptyState.svelte";
export { default as StatusBadge } from "./components/StatusBadge.svelte";
export type { StatusKind } from "./components/StatusBadge.svelte";
export { default as AppShell } from "./components/AppShell.svelte";
export { default as CapabilityCard } from "./components/CapabilityCard.svelte";
export { default as ToastHost } from "./components/ToastHost.svelte";

// ── Theme system ─────────────────────────────────────────────────────────────
export { default as ThemeProvider } from "./components/ThemeProvider.svelte";
export { default as ThemeSwitcher } from "./components/ThemeSwitcher.svelte";
export { THEME_SCRIPT } from "./components/ThemeScript.js";

// ── Features ─────────────────────────────────────────────────────────────────
export { default as AgentChatStream } from "./features/AgentChatStream.svelte";
export { default as HostedProjectCard } from "./features/HostedProjectCard.svelte";
export { default as ToolCallCard } from "./features/ToolCallCard.svelte";
// Phase 4.7: WorkspaceTree is the canonical name (was WorkspaceExplorer).
export { default as WorkspaceTree } from "./features/WorkspaceTree.svelte";
export { default as SuggestionChips } from "./features/SuggestionChips.svelte";
export { default as ContextChip } from "./features/ContextChip.svelte";
export { default as CapabilityRow } from "./features/CapabilityRow.svelte";
export { default as CapabilityBrowser } from "./features/CapabilityBrowser.svelte";
export type { CapEntry } from "./features/CapabilityBrowser.svelte";
export { default as ProfileSheet } from "./features/ProfileSheet.svelte";
// Phase 3.5: AttachmentSheet — moved from apps/browser-shell to packages/ui
export { default as AttachmentSheet } from "./features/AttachmentSheet.svelte";

// ── Page-level primitives (Phase 4) ─────────────────────────────────────────
export { default as PageHeader } from "./components/PageHeader.svelte";
export { default as DataTable } from "./components/DataTable.svelte";
export type { Column as DataTableColumn } from "./components/DataTable.types.js";
export { default as Breadcrumbs } from "./components/Breadcrumbs.svelte";
export type { BreadcrumbItem } from "./components/Breadcrumbs.svelte";

// ── Chat primitives (Phase 4.2) ──────────────────────────────────────────────
export { default as ThinkingIndicator } from "./components/ThinkingIndicator.svelte";
export { default as MessageBubble } from "./components/MessageBubble.svelte";
export type { MessageWord } from "./components/MessageBubble.svelte";
export { default as MessageList } from "./components/MessageList.svelte";
// ChatMessage is defined in MessageList (to avoid circular dep) and re-exported from AgentChatStream
export type { ChatMessage } from "./components/MessageList.svelte";
export { default as ToolCard } from "./components/ToolCard.svelte";

// ── Shell components (Phase 3) ───────────────────────────────────────────────
export { default as AppHeader } from "./components/AppHeader.svelte";
export { default as Drawer } from "./components/Drawer.svelte";
export { default as Sheet } from "./components/Sheet.svelte";
export { default as Sidebar } from "./components/Sidebar.svelte";
export { default as SidebarSection } from "./components/SidebarSection.svelte";
export { default as SidebarItem } from "./components/SidebarItem.svelte";
export { default as Composer } from "./components/Composer.svelte";
export type { Attachment } from "./components/Composer.svelte";

// ── Chrome ──────────────────────────────────────────────────────────────────
// Phase 4 close: AppTopBar/AppDrawer/AppBottomSheet/AgentChatComposer shims deleted.
// Use canonical names: AppHeader, Drawer, Sheet, Composer.

// ── Screens ─────────────────────────────────────────────────────────────────
export { default as ChatScreen } from "./features/screens/ChatScreen.svelte";
export { default as CapabilitiesScreen } from "./features/screens/CapabilitiesScreen.svelte";
export { default as CapabilityDetailSheet } from "./features/screens/CapabilityDetailSheet.svelte";
export { default as ArtifactsScreen } from "./features/screens/ArtifactsScreen.svelte";
export { default as ArtifactRow } from "./features/screens/ArtifactRow.svelte";
export { buildInvocationPrompt } from "./features/screens/buildInvocationPrompt.js";
export type { ToolCardEntry } from "./features/AgentChatStream.svelte";
// Note: Attachment is now exported from Composer.svelte (Phase 3.5) — see Shell components section above.

// ── Utils ────────────────────────────────────────────────────────────────────
export { default as LiveAnnouncer } from "./utils/LiveAnnouncer.svelte";
export { createI18n, setI18n, getI18n, t, enMessages } from "./utils/i18n.js";
export type { I18nMessages, I18nInstance } from "./utils/i18n.js";
export { autoGrow } from "./utils/actions.js";
export { prefersReducedMotion } from "./utils/motion-prefs.js";
export {
  getPlatform,
  isTauriRuntime,
  isIOSWebView,
  isAndroidWebView,
  isMacOSDesktop,
  isWindowsDesktop,
  isLinuxDesktop,
  supportsHaptics,
  supportsSafeAreaEnv,
  supportsViewTransitions,
  supportsWebShare,
  PLATFORM_SCRIPT,
} from "./utils/platform.js";
export { haptics } from "./utils/haptics.js";
export type { HapticsAPI } from "./utils/haptics.js";
export { registerKeyboardShortcuts, focusOnSlash } from "./utils/keyboard.js";
export type { KeyboardShortcutHandlers } from "./utils/keyboard.js";

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
export type { Theme, ThemePreference, ThemeAdapter, ThemeStore } from "./stores/themeStore.svelte.js";
export { createFeatureFlags } from "./stores/featureFlags.svelte.js";
export type { FeatureFlags, FeatureFlagStore } from "./stores/featureFlags.svelte.js";
export { toasts } from "./stores/toast.svelte.js";
export type { Toast, ToastKind } from "./stores/toast.svelte.js";
export { modeStore } from "./stores/modeStore.svelte.js";
export type { AppMode } from "./stores/modeStore.svelte.js";
export { recentsStore } from "./stores/recents.svelte.js";
export { breadcrumbsStore } from "./stores/breadcrumbs.svelte.js";
export { screenStore } from "./stores/screen.svelte.js";
export type { Screen } from "./stores/screen.svelte.js";
export { drawerStore } from "./stores/drawer.svelte.js";

// ── Routing ──────────────────────────────────────────────────────────────────
export { initialRoute } from "./routing/initialRoute.js";
export type { InitialRoute } from "./routing/initialRoute.js";
export { applyInitialRoute } from "./routing/applyInitialRoute.js";
export type { ApplyInitialRouteHandlers } from "./routing/applyInitialRoute.js";

// ── Capabilities ─────────────────────────────────────────────────────────────
export { createCapabilityRendererRegistry } from "./capabilities/CapabilityRendererRegistry.js";
export type { CapabilityRendererRegistry, CreateRegistryOpts } from "./capabilities/CapabilityRendererRegistry.js";
export { provideCapabilityRendererRegistry, useCapabilityRendererRegistry } from "./capabilities/CapabilityRendererRegistry.svelte.js";
