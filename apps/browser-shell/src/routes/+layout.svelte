<script lang="ts">
  import "@conusai/ui/src/lib/tokens.css";
  import { AppShell, TabStrip, RecorderControls, ToastHost } from "@conusai/ui";
  import type { Tab, Toast } from "@conusai/ui";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import type { UserStep } from "@conusai/types";

  let tabs = $state<Tab[]>([]);
  let activeTabId = $state<string | undefined>(undefined);
  let recorderState = $state<"idle" | "recording" | "uploading">("idle");
  let stepCount = $state(0);
  let toasts = $state<Toast[]>([]);
  let shellReady = $state(false);

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

  async function handleNewTab() {
    try {
      const id = await invoke<string>("create_tab", { url: "https://example.com" });
      tabs = [...tabs, { id, label: "New Tab", url: "https://example.com" }];
      activeTabId = id;
    } catch (e) {
      addToast(`Failed to open tab: ${e}`, "error");
    }
  }

  async function handleCloseTab(id: string) {
    await invoke("close_tab", { id });
    tabs = tabs.filter((t) => t.id !== id);
    if (activeTabId === id) activeTabId = tabs[0]?.id;
  }

  async function handleStartRecording() {
    await invoke("recorder_start");
    recorderState = "recording";
    stepCount = 0;

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
    recorderState = "uploading";
    try {
      const trace = await invoke("recorder_stop");
      if (trace) {
        addToast("Trace saved to workspace", "success");
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
