// ── Primitive components ─────────────────────────────────────────────────────
export { default as AppShell } from "./components/AppShell.svelte";
export { default as ArtifactPreview } from "./components/ArtifactPreview.svelte";
export { default as CapabilityCard } from "./components/CapabilityCard.svelte";
export { default as CommandPalette } from "./components/CommandPalette.svelte";
export { default as RecorderControls } from "./components/RecorderControls.svelte";
export { default as TabStrip } from "./components/TabStrip.svelte";
export { default as ToastHost } from "./components/ToastHost.svelte";
export { default as WorkspaceTree } from "./components/WorkspaceTree.svelte";
export type { Tab } from "./components/TabStrip.svelte";
export type { Toast } from "./components/ToastHost.svelte";

// ── Theme system ─────────────────────────────────────────────────────────────
export { default as ThemeProvider } from "./components/ThemeProvider.svelte";
export { default as ThemeSwitcher } from "./components/ThemeSwitcher.svelte";
export { THEME_SCRIPT } from "./components/ThemeScript.js";

// ── Features ─────────────────────────────────────────────────────────────────
export { default as AgentChatComposer } from "./features/AgentChatComposer.svelte";
export { default as AgentChatStream } from "./features/AgentChatStream.svelte";
export { default as ToolCallCard } from "./features/ToolCallCard.svelte";
export { default as WorkspaceExplorer } from "./features/WorkspaceExplorer.svelte";
export type { ChatMessage, ToolCardEntry } from "./features/AgentChatStream.svelte";
export type { Attachment } from "./features/AgentChatComposer.svelte";

// ── Utils ────────────────────────────────────────────────────────────────────
export { default as LiveAnnouncer } from "./utils/LiveAnnouncer.svelte";
export { autoGrow } from "./utils/actions.js";

// ── Stores ───────────────────────────────────────────────────────────────────
export { createThemeStore, localStorageAdapter } from "./stores/themeStore.svelte.js";
export type { Theme, ThemeAdapter, ThemeStore } from "./stores/themeStore.svelte.js";
export { createFeatureFlags } from "./stores/featureFlags.svelte.js";
export type { FeatureFlags, FeatureFlagStore } from "./stores/featureFlags.svelte.js";
export { toasts } from "./stores/toast.svelte.js";
export type { ToastKind } from "./stores/toast.svelte.js";
export { modeStore } from "./stores/modeStore.svelte.js";
export type { AppMode } from "./stores/modeStore.svelte.js";

// ── Capabilities ─────────────────────────────────────────────────────────────
export { createCapabilityRendererRegistry } from "./capabilities/CapabilityRendererRegistry.js";
export type { CapabilityRendererRegistry, CreateRegistryOpts } from "./capabilities/CapabilityRendererRegistry.js";
export { provideCapabilityRendererRegistry, useCapabilityRendererRegistry } from "./capabilities/CapabilityRendererRegistry.svelte.js";
