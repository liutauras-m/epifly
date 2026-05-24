<svelte:options runes={true} />
<script lang="ts">
  /**
   * SidebarItem — single nav row in the Sidebar (Phase 3.4).
   *
   * Renders as <a> or <button> depending on whether href is provided.
   * Shows icon-only in medium density, icon + label in expanded.
   *
   * Usage:
   *   <SidebarItem href="/chat" icon={MessageSquare}>Chat</SidebarItem>
   *   <SidebarItem icon={Plus} onclick={newChat}>New chat</SidebarItem>
   */
  import type { Component, Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    href,
    icon,
    active  = false,
    children,
    class: cls = '',
    onclick,
    ...rest
  }: {
    href?:     string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    icon?:     Component<any>;
    active?:   boolean;
    children?: Snippet;
    class?:    string;
    onclick?:  (e: MouseEvent) => void;
    [key: string]: unknown;
  } = $props();
</script>

{#if href}
  <li>
    <a
      {href}
      class="sidebar-item{active ? ' active' : ''}{cls ? ` ${cls}` : ''}"
      aria-current={active ? 'page' : undefined}
      {...rest}
    >
      {#if icon}<Icon {icon} size="md" />{/if}
      <span class="item-label">
        {#if children}{@render children()}{/if}
      </span>
    </a>
  </li>
{:else}
  <li>
    <button
      type="button"
      class="sidebar-item{active ? ' active' : ''}{cls ? ` ${cls}` : ''}"
      aria-pressed={active || undefined}
      {onclick}
      {...rest}
    >
      {#if icon}<Icon {icon} size="md" />{/if}
      <span class="item-label">
        {#if children}{@render children()}{/if}
      </span>
    </button>
  </li>
{/if}

<style>
  .sidebar-item {
    display:        flex;
    align-items:    center;
    gap:            var(--space-3);
    width:          100%;
    padding:        var(--space-2) var(--space-4);
    min-height:     var(--hit, 44px);

    background:     transparent;
    border:         none;
    border-radius:  var(--radius-sm);
    cursor:         pointer;
    text-decoration: none;
    color:          var(--color-fg-muted);
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-meta);   /* 13px */
    font-weight:    450;
    line-height:    1.3;
    white-space:    nowrap;
    overflow:       hidden;

    transition:
      background  var(--duration-fast) var(--ease-standard),  /* [feedback] */
      color       var(--duration-fast) var(--ease-standard);   /* [feedback] */

    outline: none;
  }

  .sidebar-item:hover {
    background: var(--color-bg-hover);
    color:      var(--color-fg);
  }

  .sidebar-item:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .sidebar-item.active {
    background: var(--color-accent-soft);
    color:      var(--color-accent);
    font-weight: 520;
  }

  /* Label hides in icon-only mode (medium breakpoint: 768–1023px) */
  .item-label {
    flex:        1;
    min-width:   0;
    overflow:    hidden;
    text-overflow: ellipsis;
    transition:  opacity   var(--duration-fast) var(--ease-standard),   /* [continuity] */
                 max-width var(--duration-fast) var(--ease-standard);   /* [continuity] */
  }

  @container app-shell (max-width: 1023px) {
    .sidebar-item {
      justify-content: center;
      padding: var(--space-2);
    }
    .item-label {
      display: none;
    }
  }
</style>
