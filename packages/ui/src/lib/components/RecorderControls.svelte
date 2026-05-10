<script lang="ts">
  type RecorderState = "idle" | "recording" | "uploading";

  interface Props {
    state?: RecorderState;
    stepCount?: number;
    onstart?: () => void;
    onstop?: () => void;
  }

  let { state = "idle", stepCount = 0, onstart, onstop }: Props = $props();
</script>

<div class="recorder" aria-live="polite">
  {#if state === "idle"}
    <button class="btn-record" onclick={() => onstart?.()} aria-label="Start recording">
      <span class="dot idle" aria-hidden="true"></span>
      Record
    </button>
  {:else if state === "recording"}
    <button class="btn-stop" onclick={() => onstop?.()} aria-label="Stop recording">
      <span class="dot recording" aria-hidden="true"></span>
      Stop · {stepCount} steps
    </button>
  {:else}
    <button class="btn-uploading" disabled aria-label="Uploading trace">
      <span class="dot uploading" aria-hidden="true"></span>
      Uploading…
    </button>
  {/if}
</div>

<style>
  .recorder {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
  }

  button {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-4);
    min-height: 44px;
    border-radius: 6px;
    border: 1px solid var(--rule);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    transition: background var(--duration-short) var(--ease-out);
  }

  button:disabled { opacity: 0.6; cursor: default; }

  .btn-record { background: var(--paper-2); color: var(--ink); }
  .btn-record:hover { background: var(--ember-soft); border-color: var(--ember); }

  .btn-stop { background: var(--danger-soft); color: var(--danger); border-color: var(--danger); }
  .btn-stop:hover { background: var(--danger); color: var(--paper); }

  .btn-uploading { background: var(--ember-soft); color: var(--ink-2); border-color: var(--ember); }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot.idle { background: var(--ink-3); }
  .dot.recording { background: var(--danger); animation: pulse 1.2s ease-in-out infinite; }
  .dot.uploading { background: var(--ember); animation: pulse 1.2s ease-in-out infinite; }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }
</style>
