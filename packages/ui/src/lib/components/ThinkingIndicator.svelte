<svelte:options runes={true} />
<script lang="ts">
  /**
   * ThinkingIndicator — animated sonar rings shown while the AI is generating (Phase 4.2).
   *
   * Renders an AI avatar mark + two expanding sonar rings to signal that a
   * response is in progress. Shows immediately on submit; removed on first token.
   *
   * Usage:
   *   {#if inFlight}
   *     <ThinkingIndicator />
   *   {/if}
   */
</script>

<div class="thinking-row">
  <div class="ai-mark" aria-hidden="true">
    <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
      <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
      <path d="M5 8.5l2 2 4-4" stroke="currentColor" stroke-width="1.5"
            stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </div>
  <span class="sonar" role="status" aria-label="Thinking">
    <span class="sonar-ring sonar-r1"></span>
    <span class="sonar-ring sonar-r2"></span>
    <span class="sonar-core"></span>
  </span>
</div>

<style>
  .thinking-row {
    display:     flex;
    align-items: flex-start;
    padding:     var(--_row-v, 3px) var(--space-4);
    gap:         var(--space-2);
  }

  /* AI avatar mark */
  .ai-mark {
    width:           var(--_mark-size, 22px);
    height:          var(--_mark-size, 22px);
    border-radius:   50%;
    background:      var(--color-bg-hover);
    color:           var(--color-fg-subtle);
    display:         flex;
    align-items:     center;
    justify-content: center;
    flex-shrink:     0;
    margin-top:      var(--_mark-mt, 2px);
  }

  /* Sonar ring animation */
  .sonar {
    display:     inline-flex;
    position:    relative;
    width:       var(--_sonar-size, 14px);
    height:      var(--_sonar-size, 14px);
    flex-shrink: 0;
    align-self:  center;
  }
  .sonar-core {
    position:      absolute;
    inset:         25%;
    border-radius: 50%;
    background:    var(--color-accent);
  }
  .sonar-ring {
    position:      absolute;
    inset:         0;
    border-radius: 50%;
    border:        1.5px solid var(--color-accent);
    animation:     sonar-out 1.8s ease-out infinite;  /* [feedback] thinking in progress */
  }
  .sonar-r2 { animation-delay: 0.6s; }
  @keyframes sonar-out {
    0%   { transform: scale(0.3); opacity: 0.9; }
    100% { transform: scale(2.2); opacity: 0; }
  }

  @media (prefers-reduced-motion: reduce) {
    .sonar-ring { animation: none; opacity: 0.6; }
  }
</style>
