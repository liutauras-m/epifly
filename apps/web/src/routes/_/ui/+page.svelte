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
  import Icon               from '@conusai/ui/components/Icon.svelte';
  import Button             from '@conusai/ui/components/Button.svelte';
  import Field              from '@conusai/ui/components/Field.svelte';
  import Chip               from '@conusai/ui/components/Chip.svelte';
  import EmptyState         from '@conusai/ui/components/EmptyState.svelte';
  import StatusBadge        from '@conusai/ui/components/StatusBadge.svelte';
  import Composer           from '@conusai/ui/components/Composer.svelte';
  import PlanBadge          from '@conusai/ui/components/PlanBadge.svelte';
  import PlanCard           from '@conusai/ui/components/PlanCard.svelte';
  import UsageMeter         from '@conusai/ui/components/UsageMeter.svelte';
  import CapabilityCard     from '@conusai/ui/components/CapabilityCard.svelte';
  import ThemeSwitcher      from '@conusai/ui/components/ThemeSwitcher.svelte';
  import ToastHost          from '@conusai/ui/components/ToastHost.svelte';
  import QuotaBanner        from '@conusai/ui/components/QuotaBanner.svelte';
  // ── Phase 3 shell primitives ──────────────────────────────────────────────
  import AppShell           from '@conusai/ui/components/AppShell.svelte';
  import AppHeader          from '@conusai/ui/components/AppHeader.svelte';
  import Sidebar            from '@conusai/ui/components/Sidebar.svelte';
  import SidebarSection     from '@conusai/ui/components/SidebarSection.svelte';
  import SidebarItem        from '@conusai/ui/components/SidebarItem.svelte';
  import Drawer             from '@conusai/ui/components/Drawer.svelte';
  import Sheet              from '@conusai/ui/components/Sheet.svelte';
  import WorkspaceTree      from '@conusai/ui/features/WorkspaceTree.svelte';
  import ThemeProvider      from '@conusai/ui/components/ThemeProvider.svelte';
  import { toasts }         from '@conusai/ui/stores';
  import { getContext }     from 'svelte';

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type AnyComponent = Component<any>;
  // The theme context is provided by the root ThemeProvider in +layout.svelte.
  const theme = getContext<{ current: string; toggle: () => void }>('conusai.theme');

  // ── Fixtures ──────────────────────────────────────────────────────────────
  import typeFx              from '@conusai/ui/components/Type.fixtures.js';
  import iconFx              from '@conusai/ui/components/Icon.fixtures.js';
  import buttonFx            from '@conusai/ui/components/Button.fixtures.js';
  import fieldFx             from '@conusai/ui/components/Field.fixtures.js';
  import chipFx              from '@conusai/ui/components/Chip.fixtures.js';
  import emptyStateFx        from '@conusai/ui/components/EmptyState.fixtures.js';
  import statusBadgeFx       from '@conusai/ui/components/StatusBadge.fixtures.js';
  import composerFx2         from '@conusai/ui/components/Composer.fixtures.js';
  import planBadgeFx         from '@conusai/ui/components/PlanBadge.fixtures.js';
  import planCardFx          from '@conusai/ui/components/PlanCard.fixtures.js';
  import usageMeterFx        from '@conusai/ui/components/UsageMeter.fixtures.js';
  import capabilityCardFx    from '@conusai/ui/components/CapabilityCard.fixtures.js';
  import themeSwitcherFx     from '@conusai/ui/components/ThemeSwitcher.fixtures.js';
  import toastHostFx         from '@conusai/ui/components/ToastHost.fixtures.js';
  import quotaBannerFx       from '@conusai/ui/components/QuotaBanner.fixtures.js';
  import appShellFx          from '@conusai/ui/components/AppShell.fixtures.js';
  import appHeaderFx         from '@conusai/ui/components/AppHeader.fixtures.js';
  import sidebarFx           from '@conusai/ui/components/Sidebar.fixtures.js';
  import sidebarSectionFx    from '@conusai/ui/components/SidebarSection.fixtures.js';
  import sidebarItemFx       from '@conusai/ui/components/SidebarItem.fixtures.js';
  import drawerFx            from '@conusai/ui/components/Drawer.fixtures.js';
  import sheetFx             from '@conusai/ui/components/Sheet.fixtures.js';
  import workspaceTreeFx     from '@conusai/ui/features/WorkspaceTree.fixtures.js';
  import themeProviderFx     from '@conusai/ui/components/ThemeProvider.fixtures.js';
  // ── Phase 4 primitives ────────────────────────────────────────────────────
  import PageHeader          from '@conusai/ui/components/PageHeader.svelte';
  import DataTable           from '@conusai/ui/components/DataTable.svelte';
  import Breadcrumbs         from '@conusai/ui/components/Breadcrumbs.svelte';
  import ThinkingIndicator   from '@conusai/ui/components/ThinkingIndicator.svelte';
  import MessageBubble       from '@conusai/ui/components/MessageBubble.svelte';
  import MessageList         from '@conusai/ui/components/MessageList.svelte';
  import ToolCard            from '@conusai/ui/components/ToolCard.svelte';
  import pageHeaderFx        from '@conusai/ui/components/PageHeader.fixtures.js';
  import dataTableFx         from '@conusai/ui/components/DataTable.fixtures.js';
  import breadcrumbsFx       from '@conusai/ui/components/Breadcrumbs.fixtures.js';
  import thinkingIndicatorFx from '@conusai/ui/components/ThinkingIndicator.fixtures.js';
  import messageBubbleFx     from '@conusai/ui/components/MessageBubble.fixtures.js';
  import messageListFx       from '@conusai/ui/components/MessageList.fixtures.js';
  import toolCardFx          from '@conusai/ui/components/ToolCard.fixtures.js';

  import type { Component } from 'svelte';
  import type { ComponentFixtureSet } from '@conusai/ui/gallery.types';

  // ── Registry ──────────────────────────────────────────────────────────────
  interface RegistryEntry {
    name:        string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    component:   Component<any>;
    fixtures:    ComponentFixtureSet;
    fullWidth?:  boolean;    // stretch to container width
    deprecated?: boolean;    // @deprecated alias — rendered with a badge
  }

  const REGISTRY: RegistryEntry[] = [
    // ── Phase 2.2-2.4 primitives ──────────────────────────────────────────
    { name: 'Type',              component: Type,              fixtures: typeFx                            },
    { name: 'Icon',              component: Icon,              fixtures: iconFx                            },
    // ── Phase 2.7 cross-cutting primitives ───────────────────────────────
    { name: 'Button',            component: Button,            fixtures: buttonFx                          },
    { name: 'Field',             component: Field,             fixtures: fieldFx                           },
    { name: 'Chip',              component: Chip,              fixtures: chipFx                            },
    { name: 'EmptyState',        component: EmptyState,        fixtures: emptyStateFx,   fullWidth: true   },
    { name: 'StatusBadge',       component: StatusBadge,       fixtures: statusBadgeFx                     },
    { name: 'Composer',          component: Composer,          fixtures: composerFx2,    fullWidth: true   },
    // ── Phase 4 page primitives ───────────────────────────────────────────
    { name: 'PageHeader',        component: PageHeader,        fixtures: pageHeaderFx,   fullWidth: true   },
    { name: 'DataTable',         component: DataTable,         fixtures: dataTableFx,    fullWidth: true   },
    { name: 'Breadcrumbs',       component: Breadcrumbs,       fixtures: breadcrumbsFx                     },
    // ── Phase 4.2 chat primitives ─────────────────────────────────────────
    { name: 'ThinkingIndicator', component: ThinkingIndicator, fixtures: thinkingIndicatorFx               },
    { name: 'MessageBubble',     component: MessageBubble,     fixtures: messageBubbleFx                   },
    { name: 'MessageList',       component: MessageList,       fixtures: messageListFx,  fullWidth: true   },
    { name: 'ToolCard',          component: ToolCard,          fixtures: toolCardFx,     fullWidth: true   },
    // ── Phase 3 shell primitives ──────────────────────────────────────────
    { name: 'AppShell',          component: AppShell,          fixtures: appShellFx,     fullWidth: true   },
    { name: 'AppHeader',         component: AppHeader,         fixtures: appHeaderFx,    fullWidth: true   },
    { name: 'Sidebar',           component: Sidebar,           fixtures: sidebarFx,      fullWidth: true   },
    { name: 'SidebarSection',    component: SidebarSection,    fixtures: sidebarSectionFx                  },
    { name: 'SidebarItem',       component: SidebarItem,       fixtures: sidebarItemFx                     },
    { name: 'Drawer',            component: Drawer,            fixtures: drawerFx,       fullWidth: true   },
    { name: 'Sheet',             component: Sheet,             fixtures: sheetFx,        fullWidth: true   },
    { name: 'WorkspaceTree',     component: WorkspaceTree,     fixtures: workspaceTreeFx                   },
    // ── Theme / meta ──────────────────────────────────────────────────────
    { name: 'ThemeProvider',     component: ThemeProvider,     fixtures: themeProviderFx                   },
    { name: 'ThemeSwitcher',     component: ThemeSwitcher,     fixtures: themeSwitcherFx                   },
    // ── Billing components ────────────────────────────────────────────────
    { name: 'PlanBadge',         component: PlanBadge,         fixtures: planBadgeFx                      },
    { name: 'UsageMeter',        component: UsageMeter,        fixtures: usageMeterFx,   fullWidth: true   },
    { name: 'PlanCard',          component: PlanCard,          fixtures: planCardFx,     fullWidth: true   },
    { name: 'CapabilityCard',    component: CapabilityCard,    fixtures: capabilityCardFx                  },
    { name: 'QuotaBanner',       component: QuotaBanner,       fixtures: quotaBannerFx,  fullWidth: true   },
    { name: 'ToastHost',         component: ToastHost,         fixtures: toastHostFx,    fullWidth: true   },
    // Phase 4 close: AppTopBar/AppDrawer/AppBottomSheet/AgentChatComposer shims deleted.
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
            class:deprecated={r.deprecated}
            onclick={() => activeSection = r.name}
          >
            {r.name}
            {#if r.deprecated}<span class="dep-badge" aria-label="deprecated">@dep</span>{/if}
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
        <h1 class="component-name">
          {entry.name}
          {#if entry.deprecated}<span class="deprecated-notice">@deprecated — delete at Phase 4 close</span>{/if}
        </h1>
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
    background: var(--color-bg);
    color: var(--color-fg);
  }

  /* ── Sidebar ─────────────────────────────────────────────────────────────── */
  .sidebar {
    border-right: 1px solid var(--color-border);
    padding: var(--space-4) 0;
    position: sticky;
    top: 0;
    height: 100dvh;
    overflow-y: auto;
    background: var(--color-bg-raised);
  }
  .sidebar-header {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    padding: 0 var(--space-4) var(--space-3);
    border-bottom: 1px solid var(--color-border);
    margin-bottom: var(--space-2);
  }
  .eyebrow {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--color-fg-subtle);
  }
  .count {
    font-size: 11px;
    color: var(--color-fg-subtle);
    background: var(--color-bg-hover);
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
    color: var(--color-fg-muted);
    cursor: pointer;
    transition: background var(--duration-fast);
  }
  .nav-item:hover  { background: var(--color-bg-hover); }
  .nav-item.active { background: var(--color-accent); color: var(--color-on-accent); font-weight: 500; }

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
    color: var(--color-fg);
  }
  .note {
    font-size: 13px;
    color: var(--color-fg-subtle);
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
    color: var(--color-fg-subtle);
  }
  .toolbar-sep {
    width: 1px;
    height: 16px;
    background: var(--color-border);
    margin: 0 var(--space-1);
  }
  .toggle-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--color-fg-muted);
    cursor: pointer;
    padding: 3px 10px;
    border: 1px solid var(--color-border);
    border-radius: 99px;
    background: var(--color-bg);
    transition: background var(--duration-fast), color var(--duration-fast);
  }
  .toggle-chip:has(input:checked) {
    background: var(--color-fg);
    color: var(--color-bg);
    border-color: transparent;
  }
  .toggle-chip input { display: none; }

  .chip-btn {
    font-size: 11px;
    padding: 3px 10px;
    border-radius: 99px;
    border: 1px solid var(--color-border);
    background: transparent;
    cursor: pointer;
    color: var(--color-fg-muted);
    transition: background var(--duration-fast);
  }
  .chip-btn.success { border-color: var(--success); color: var(--success); }
  .chip-btn.error   { border-color: var(--danger);  color: var(--danger);  }
  .chip-btn.warning { border-color: var(--warning, #d97706); color: var(--warning, #d97706); }

  .theme-pill {
    margin-left: auto;
    font-size: 11px;
    font-family: var(--font-mono, ui-monospace, monospace);
    color: var(--color-fg-subtle);
    background: var(--color-bg-hover);
    padding: 2px 8px;
    border-radius: 99px;
    border: 1px solid var(--color-border);
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
    color: var(--color-fg-subtle);
    font-family: var(--font-mono, ui-monospace, monospace);
  }

  .fixture-stage {
    padding: var(--space-4);
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    display: flex;
    align-items: flex-start;
    justify-content: flex-start;
  }
  .fixture-frame.full-width .fixture-stage { display: block; }

  /* ── Deprecated component indicators ───────────────────────────────────── */
  .nav-item.deprecated { opacity: 0.55; }
  .nav-item.deprecated:hover { opacity: 0.85; }
  .dep-badge {
    display: inline-block;
    margin-left: var(--space-1);
    font-size: 9px;
    font-family: var(--font-mono, ui-monospace, monospace);
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(217, 119, 6, 0.15);
    color: var(--color-warning, #d97706);
    vertical-align: middle;
  }
  .deprecated-notice {
    display: inline-block;
    margin-left: var(--space-3);
    font-size: 12px;
    font-weight: 400;
    color: var(--color-warning, #d97706);
    font-family: var(--font-mono, ui-monospace, monospace);
    background: rgba(217, 119, 6, 0.10);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-xs);
  }
</style>
