<script lang="ts">
  import "@conusai/ui/src/lib/tokens.css";
  import { AppShell, TabStrip, RecorderControls, ToastHost } from "@conusai/ui";
  import type { Tab, Toast } from "@conusai/ui";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import type { SessionTrace } from "@conusai/types";

  let tabs = $state<Tab[]>([]);
  let activeTabId = $state<string | undefined>(undefined);
  let recorderState = $state<"idle" | "recording" | "uploading">("idle");
  let stepCount = $state(0);
  let toasts = $state<Toast[]>([]);
  let shellReady = $state(false);

  // Screenshot polling: capture once per second while recording.
  let screenshotInterval: ReturnType<typeof setInterval> | undefined;

  function addToast(message: string, kind: Toast["kind"] = "info") {
    const id = crypto.randomUUID();
    toasts = [...toasts, { id, message, kind }];
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
    }, 4000);
  }

  onMount(() => {
    // Rust emits shell-ready after startup tasks; we load the device token from
    // Stronghold so Rust can use it for capability registration.
    const unlistenPromise = listen("shell-ready", async () => {
      shellReady = true;
      await loadTokenFromStronghold();
      await restorePersistedTabs();
    });

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  });

  async function loadTokenFromStronghold() {
    try {
      const { Client } = await import("@tauri-apps/plugin-stronghold");
      const { appDataDir } = await import("@tauri-apps/api/path");
      const vaultPath = (await appDataDir()) + "/conusai.stronghold";
      const client = await Client.load(vaultPath, "conusai-shell-v1");
      const store = client.getStore("tokens");
      const raw = await store.get("device_token");
      if (raw) {
        const token = new TextDecoder().decode(new Uint8Array(raw));
        await invoke("set_device_token", { token });
      }
    } catch {
      // Stronghold vault not yet provisioned — token falls back to env var set at launch.
    }
  }

  async function restorePersistedTabs() {
    try {
      const saved = await invoke<Tab[]>("restore_tabs");
      for (const tab of saved) {
        const id = await invoke<string>("create_tab", { url: tab.url });
        tabs = [...tabs, { id, label: tab.label, url: tab.url }];
        if (!activeTabId) activeTabId = id;
      }
    } catch {
      // No persisted tabs — fresh start.
    }
  }

  async function handleNewTab() {
    try {
      const id = await invoke<string>("create_tab", { url: "https://example.com" });
      tabs = [...tabs, { id, label: "New Tab", url: "https://example.com" }];
      activeTabId = id;
      await invoke("save_tabs");
    } catch (e) {
      addToast(`Failed to open tab: ${e}`, "error");
    }
  }

  async function handleCloseTab(id: string) {
    await invoke("close_tab", { id });
    tabs = tabs.filter((t) => t.id !== id);
    if (activeTabId === id) activeTabId = tabs[0]?.id;
    await invoke("save_tabs");
  }

  function startScreenshotPolling() {
    if (!activeTabId) return;
    const tabId = activeTabId;
    screenshotInterval = setInterval(async () => {
      if (recorderState !== "recording") {
        stopScreenshotPolling();
        return;
      }
      try {
        // Capture screenshot of active tab; Rust returns base64 PNG.
        // The recorder_record_step bridge in the tab webview already filters PII fields;
        // this screenshot is stored alongside the current step (best-effort).
        await invoke("capture_tab_screenshot", { tabId });
      } catch {
        // Screenshot capture is best-effort — ignore failures silently.
      }
    }, 1000);
  }

  function stopScreenshotPolling() {
    if (screenshotInterval !== undefined) {
      clearInterval(screenshotInterval);
      screenshotInterval = undefined;
    }
  }

  async function handleStartRecording() {
    await invoke("recorder_start");
    recorderState = "recording";
    stepCount = 0;
    startScreenshotPolling();

    const interval = setInterval(async () => {
      if (recorderState !== "recording") {
        clearInterval(interval);
        return;
      }
      const status = await invoke<{ recording: boolean; step_count: number }>(
        "recorder_status",
      );
      stepCount = status.step_count;
    }, 500);
  }

  async function handleStopRecording() {
    stopScreenshotPolling();
    recorderState = "uploading";
    try {
      const trace = await invoke<SessionTrace | null>("recorder_stop");
      if (trace) {
        // ArtifactBridge upload: POST /v1/files then POST /v1/workspaces.
        const nodeId = await invoke<string>("upload_trace_cmd", { trace });
        addToast(`Trace saved — workspace node ${nodeId.slice(0, 8)}…`, "success");
      }
    } catch (e) {
      addToast(`Upload failed: ${e}`, "error");
    } finally {
      recorderState = "idle";
      stepCount = 0;
    }
  }

  let { children } = $props();
</script>

<AppShell title="ConusAI Browser">
  {#snippet sidebar()}
    <div class="shell-sidebar">
      <div class="sidebar-header">
        <span class="logo">ConusAI</span>
        {#if !shellReady}
          <span class="status-dot loading" title="Connecting…"></span>
        {:else}
          <span class="status-dot ready" title="Connected"></span>
        {/if}
      </div>
      <RecorderControls
        state={recorderState}
        {stepCount}
        onstart={handleStartRecording}
        onstop={handleStopRecording}
      />
    </div>
  {/snippet}

  <div class="shell-content">
    <TabStrip
      {tabs}
      {activeTabId}
      onselect={(id) => (activeTabId = id)}
      onclose={handleCloseTab}
      oncreate={handleNewTab}
    />
    <div class="page">
      {@render children()}
    </div>
  </div>
</AppShell>

<ToastHost {toasts} ondismiss={(id) => (toasts = toasts.filter((t) => t.id !== id))} />

<style>
  .shell-sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-4);
    border-bottom: 1px solid var(--rule);
  }

  .logo {
    font-family: var(--font-display);
    font-size: 18px;
    color: var(--ember);
    flex: 1;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .status-dot.loading {
    background: var(--ink-muted);
    animation: pulse 1.2s ease-in-out infinite;
  }

  .status-dot.ready {
    background: #22c55e;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .shell-content {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .page {
    flex: 1;
    overflow: auto;
  }
</style>
