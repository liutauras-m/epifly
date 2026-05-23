<svelte:options runes={true} />
<script lang="ts">
  /**
   * /_/ui — Primitive gallery (Phase 2.6)
   *
   * Dev-only route (guarded by +layout.ts).  Renders every component in
   * packages/ui/src/lib/components/ with its fixture sets in isolation.
   * Supports viewport-size preview and live theme toggling.
   *
   * Adding a new primitive:
   *   1. Create Component.fixtures.ts next to Component.svelte in packages/ui.
   *   2. Add an entry to the REGISTRY array below.
   *   3. Run `just web-dev` and visit /_/ui.
   */

  // ── Primitives ────────────────────────────────────────────────────────────
  import Type               from '@conusai/ui/components/Type.svelte';
  import PlanBadge          from '@conusai/ui/components/PlanBadge.svelte';
  import PlanCard           from '@conusai/ui/components/PlanCard.svelte';
  import UsageMeter         from '@conusai/ui/components/UsageMeter.svelte';
  import CapabilityCard     from '@conusai/ui/components/CapabilityCard.svelte';
  import AgentChatComposer  from '@conusai/ui/components/AgentChatComposer.svelte';
  import AppTopBar          from '@conusai/ui/components/AppTopBar.svelte';
  import ThemeSwitcher      from '@conusai/ui/components/ThemeSwitcher.svelte';
  import ToastHost          from '@conusai/ui/components/ToastHost.svelte';
  import QuotaBanner        from '@conusai/ui/components/QuotaBanner.svelte';
  import { toasts }         from '@conusai/ui/stores';
  import { getContext }     from 'svelte';

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type AnyComponent = Component<any>;
  // The theme context is provided by the root ThemeProvider in +layout.svelte.
  const theme = getContext<{ current: string; toggle: () => void }>('conusai.theme');

  // ── Fixtures ──────────────────────────────────────────────────────────────
  import typeFx              from '@conusai/ui/components/Type.fixtures.js';
  import planBadgeFx         from '@conusai/ui/components/PlanBadge.fixtures.js';
  import planCardFx          from '@conusai/ui/components/PlanCard.fixtures.js';
  import usageMeterFx        from '@conusai/ui/components/UsageMeter.fixtures.js';
  import capabilityCardFx    from '@conusai/ui/components/CapabilityCard.fixtures.js';
  import composerFx          from '@conusai/ui/components/AgentChatComposer.fixtures.js';
  import appTopBarFx         from '@conusai/ui/components/AppTopBar.fixtures.js';
  import themeSwitcherFx     from '@conusai/ui/components/ThemeSwitcher.fixtures.js';
  import toastHostFx         from '@conusai/ui/components/ToastHost.fixtures.js';
  import quotaBannerFx       from '@conusai/ui/components/QuotaBanner.fixtures.js';

  import type { Component } from 'svelte';
  import type { ComponentFixtureSet } from '@conusai/ui/gallery.types';

  // ── Registry ──────────────────────────────────────────────────────────────
  interface RegistryEntry {
    name:       string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    component:  Component<any>;
    fixtures:   ComponentFixtureSet;
    fullWidth?: boolean;    // stretch to container width
  }

  const REGISTRY: RegistryEntry[] = [
    { name: 'Type',              component: Type,              fixtures: typeFx                           },
    { name: 'PlanBadge',         component: PlanBadge,         fixtures: planBadgeFx                     },
    { name: 'UsageMeter',        component: UsageMeter,        fixtures: usageMeterFx,   fullWidth: true  },
    { name: 'PlanCard',          component: PlanCard,          fixtures: planCardFx,     fullWidth: true  },
    { name: 'CapabilityCard',    component: CapabilityCard,    fixtures: capabilityCardFx                 },
    { name: 'AgentChatComposer', component: AgentChatComposer, fixtures: composerFx,     fullWidth: true  },
    { name: 'AppTopBar',         component: AppTopBar,         fixtures: appTopBarFx,    fullWidth: true  },
    { name: 'ThemeSwitcher',     component: ThemeSwitcher,     fixtures: themeSwitcherFx                  },
    { name: 'ToastHost',         component: ToastHost,         fixtures: toastHostFx,    fullWidth: true  },
    { name: 'QuotaBanner',       component: QuotaBanner,       fixtures: quotaBannerFx,  fullWidth: true  },
  ];

  // ── State ─────────────────────────────────────────────────────────────────
  let activeSection = $state(REGISTRY[0].name);
  let viewportWidth = $state<'mobile' | 'desktop'>('desktop');

  const entry = $derived(REGISTRY.find(r => r.name === activeSection) ?? REGISTRY[0]);


  // Toast demo helpers
  function pushSuccess() { toasts.success('Action completed.');             }
  function pushError()   { toasts.error('Something went wrong.');           }
  function pushWarning() { toasts.warning('Quota at 90 % of daily limit.'); }
</script>

<svelte:head>
  <title>/_/ui — Primitive gallery</title>
</svelte:head>

<div class="gallery-shell">

  <!-- ── Sidebar nav ──────────────────────────────────────────────────────── -->
  <nav class="sidebar" aria-label="Component list">
    <header class="sidebar-header">
      <span class="eyebrow">Primitives</span>
      <span class="count">{REGISTRY.length}</span>
    </header>
    <ul class="component-list">
      {#each REGISTRY as r (r.name)}
        <li>
          <button
            class="nav-item"
            class:active={r.name === activeSection}
            onclick={() => activeSection = r.name}
          >
            {r.name}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <!-- ── Main canvas ──────────────────────────────────────────────────────── -->
  <main class="canvas">

    <!-- Section heading + toolbar -->
    <div class="section-header">
      <div class="heading-row">
        <h1 class="component-name">{entry.name}</h1>
        <ThemeSwitcher />
      </div>
      {#if entry.fixtures.note}
        <p class="note">{entry.fixtures.note}</p>
      {/if}

      <div class="toolbar">
        <span class="toolbar-label">Viewport</span>
        <label class="toggle-chip">
          <input type="radio" name="vp" value="desktop"
                 checked={viewportWidth === 'desktop'}
                 onchange={() => viewportWidth = 'desktop'} />
          Desktop
        </label>
        <label class="toggle-chip">
          <input type="radio" name="vp" value="mobile"
                 checked={viewportWidth === 'mobile'}
                 onchange={() => viewportWidth = 'mobile'} />
          Mobile 375 px
        </label>

        {#if entry.name === 'ToastHost'}
          <span class="toolbar-sep"></span>
          <button class="chip-btn success" onclick={pushSuccess}> + Success </button>
          <button class="chip-btn error"   onclick={pushError}>   + Error   </button>
          <button class="chip-btn warning" onclick={pushWarning}> + Warning </button>
        {/if}

        <span class="theme-pill" data-theme-pill={theme?.current ?? 'paper'}>
          {theme?.current ?? 'paper'} theme
        </span>
      </div>
    </div>

    <!-- Fixture grid -->
    <div class="fixture-grid" class:mobile={viewportWidth === 'mobile'}>
      {#each entry.fixtures.cases as fx (fx.label)}
        {@const Comp = entry.component as AnyComponent}
        <figure class="fixture-frame" class:full-width={entry.fullWidth}>
          <figcaption class="fixture-label">{fx.label}</figcaption>
          <div class="fixture-stage">
            <Comp {...fx.props} />
          </div>
        </figure>
      {/each}
    </div>

  </main>

</div>

<style>
  /* ── Shell layout ────────────────────────────────────────────────────────── */
  .gallery-shell {
    display: grid;
    grid-template-columns: 200px 1fr;
    min-height: 100dvh;
    background: var(--bg, var(--paper));
    color: var(--ink);
  }

  /* ── Sidebar ─────────────────────────────────────────────────────────────── */
  .sidebar {
    border-right: 1px solid var(--rule);
    padding: var(--space-4) 0;
    position: sticky;
    top: 0;
    height: 100dvh;
    overflow-y: auto;
    background: var(--paper-2);
  }
  .sidebar-header {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    padding: 0 var(--space-4) var(--space-3);
    border-bottom: 1px solid var(--rule);
    margin-bottom: var(--space-2);
  }
  .eyebrow {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  .count {
    font-size: 11px;
    color: var(--ink-3);
    background: var(--paper-3);
    padding: 1px 6px;
    border-radius: 99px;
  }
  .component-list { list-style: none; margin: 0; padding: 0; }
  .nav-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: var(--space-2) var(--space-4);
    background: transparent;
    border: none;
    font-size: 13px;
    color: var(--ink-2);
    cursor: pointer;
    transition: background var(--duration-fast);
  }
  .nav-item:hover  { background: var(--paper-3); }
  .nav-item.active { background: var(--ember); color: #fff; font-weight: 500; }

  /* ── Canvas ──────────────────────────────────────────────────────────────── */
  .canvas { padding: var(--space-6); overflow-y: auto; }
  .section-header { margin-bottom: var(--space-6); }

  .heading-row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin-bottom: var(--space-2);
  }
  .component-name {
    font-size: 22px;
    font-weight: 600;
    margin: 0;
    color: var(--ink);
  }
  .note {
    font-size: 13px;
    color: var(--ink-3);
    margin: 0 0 var(--space-3);
    max-width: 560px;
  }

  /* Toolbar */
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-wrap: wrap;
  }
  .toolbar-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-3);
  }
  .toolbar-sep {
    width: 1px;
    height: 16px;
    background: var(--rule);
    margin: 0 var(--space-1);
  }
  .toggle-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--ink-2);
    cursor: pointer;
    padding: 3px 10px;
    border: 1px solid var(--rule);
    border-radius: 99px;
    background: var(--paper);
    transition: background var(--duration-fast), color var(--duration-fast);
  }
  .toggle-chip:has(input:checked) {
    background: var(--ink);
    color: var(--paper);
    border-color: transparent;
  }
  .toggle-chip input { display: none; }

  .chip-btn {
    font-size: 11px;
    padding: 3px 10px;
    border-radius: 99px;
    border: 1px solid var(--rule);
    background: transparent;
    cursor: pointer;
    color: var(--ink-2);
    transition: background var(--duration-fast);
  }
  .chip-btn.success { border-color: var(--success); color: var(--success); }
  .chip-btn.error   { border-color: var(--danger);  color: var(--danger);  }
  .chip-btn.warning { border-color: var(--warning, #d97706); color: var(--warning, #d97706); }

  .theme-pill {
    margin-left: auto;
    font-size: 11px;
    font-family: var(--font-mono, ui-monospace, monospace);
    color: var(--ink-3);
    background: var(--paper-3);
    padding: 2px 8px;
    border-radius: 99px;
    border: 1px solid var(--rule);
  }

  /* ── Fixture grid ────────────────────────────────────────────────────────── */
  .fixture-grid {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-5);
    align-items: flex-start;
  }
  .fixture-grid.mobile .fixture-stage { width: 375px; overflow-x: hidden; }

  .fixture-frame {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 160px;
  }
  .fixture-frame.full-width { width: 100%; flex: 1 1 100%; }

  .fixture-label {
    font-size: 11px;
    color: var(--ink-3);
    font-family: var(--font-mono, ui-monospace, monospace);
  }

  .fixture-stage {
    padding: var(--space-4);
    background: var(--paper);
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm);
    display: flex;
    align-items: flex-start;
    justify-content: flex-start;
  }
  .fixture-frame.full-width .fixture-stage { display: block; }
</style>
