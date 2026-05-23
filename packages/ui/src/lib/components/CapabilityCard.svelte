<script lang="ts">
  import type { CapabilityCard as CapabilityCardType } from "@conusai/types";

  interface Props {
    card: CapabilityCardType;
    onclick?: (card: CapabilityCardType) => void;
  }

  let { card, onclick }: Props = $props();
</script>

<article
  class="capability-card"
  role={onclick ? "button" : undefined}
  tabindex={onclick ? 0 : undefined}
  onclick={() => onclick?.(card)}
  onkeydown={(e) => e.key === "Enter" && onclick?.(card)}
>
  <header>
    <span class="kind">{card.kind}</span>
    <h3 class="name">{card.name}</h3>
  </header>
  <p class="description">{card.description}</p>
  {#if card.tags.length}
    <ul class="tags" aria-label="Tags">
      {#each card.tags as tag (tag)}
        <li class="tag">{tag}</li>
      {/each}
    </ul>
  {/if}
</article>

<style>
  .capability-card {
    padding: var(--space-4);
    border: 1px solid var(--rule);
    border-radius: 8px;
    background: var(--paper-2);
    transition: border-color var(--duration-fast) var(--ease-out),
                box-shadow var(--duration-fast) var(--ease-out);
  }

  .capability-card[role="button"] {
    cursor: pointer;
  }

  .capability-card[role="button"]:hover,
  .capability-card[role="button"]:focus-visible {
    border-color: var(--ember);
    box-shadow: 0 0 0 3px var(--ember-glow);
    outline: none;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }

  .kind {
    font-family: var(--font-mono);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ember-2);
    background: var(--ember-soft);
    padding: 2px 6px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .name {
    margin: 0;
    font-family: var(--font-family-sans);
    font-size: 14px;
    font-weight: 600;
    color: var(--ink);
  }

  .description {
    margin: 0 0 var(--space-3);
    font-size: 13px;
    color: var(--ink-2);
    line-height: 1.5;
  }

  .tags {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .tag {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-3);
    border: 1px solid var(--rule);
    padding: 1px 6px;
    border-radius: 4px;
  }
</style>
