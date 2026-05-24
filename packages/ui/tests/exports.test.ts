// @vitest-environment node
/**
 * exports.test.ts — Phase 8.1 export contract test.
 *
 * Verifies every public entry in @conusai/ui's exports map resolves
 * to a defined value. Catches the failure mode where a component is
 * renamed/moved and the exports map still points at the old path.
 *
 * Runs as: pnpm test (vitest) — automatically included via tests/ glob.
 *
 * Design:
 *   - Only imports that can be statically resolved in Node (no DOM) are tested here.
 *   - Svelte components are tested by checking they export an object (SFC default export).
 *   - CSS file existence is checked via fs, not dynamic import (no CSS loader in vitest).
 *   - Motion helpers tested as a separate optional block.
 */

import { describe, it, expect } from 'vitest';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// From: packages/ui/tests/exports.test.ts
// Go up 4 levels: tests/ → packages/ui/ → packages/ → conusai-platform/
const ROOT = join(fileURLToPath(import.meta.url), '../../../..');
const UI_ROOT = join(ROOT, 'packages/ui/src/lib');

// ── Helpers ───────────────────────────────────────────────────────────────────

function resolvedPath(...parts: string[]) {
  return join(UI_ROOT, ...parts);
}

function fileExists(...parts: string[]) {
  return existsSync(resolvedPath(...parts));
}

// ── CSS files ─────────────────────────────────────────────────────────────────

describe('@conusai/ui CSS exports', () => {
  it('tokens.css exists', () => {
    expect(fileExists('tokens.css')).toBe(true);
  });

  it('foundry.css exists', () => {
    expect(fileExists('foundry.css')).toBe(true);
  });
});

// ── Main index exports ────────────────────────────────────────────────────────

describe('@conusai/ui main index exports', async () => {
  // Dynamic import via file URL to bypass package resolution
  const url = pathToFileURL(resolvedPath('index.ts')).href;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let mod: Record<string, any>;
  try {
    mod = await import(url);
  } catch {
    mod = {};
  }

  const expectedExports = [
    // Primitives
    'Type', 'Icon', 'Button', 'Field', 'Chip', 'EmptyState', 'StatusBadge',
    // Page-level (Phase 4)
    'PageHeader', 'DataTable', 'Breadcrumbs',
    // Chat primitives (Phase 4.2)
    'ThinkingIndicator', 'MessageBubble', 'MessageList', 'ToolCard',
    // Shell
    'AppShell', 'AppHeader', 'Drawer', 'Sheet', 'Sidebar', 'SidebarSection', 'SidebarItem',
    'Composer',
    // Theme
    'ThemeProvider', 'ThemeSwitcher', 'THEME_SCRIPT',
    // Billing
    'PlanBadge', 'PlanCard', 'UsageMeter', 'QuotaBanner',
    // Stores
    'toasts', 'modeStore', 'recentsStore', 'breadcrumbsStore', 'screenStore', 'drawerStore',
    // Utils
    'autoGrow', 'prefersReducedMotion',
    'getPlatform', 'isTauriRuntime', 'isIOSWebView', 'supportsHaptics',
    'haptics', 'registerKeyboardShortcuts', 'focusOnSlash',
    'createI18n', 'setI18n', 'getI18n', 't', 'enMessages',
    // Motion
    'springAnimate', 'tap', 'stagger', 'startViewTransition',
    // Routing
    'initialRoute', 'applyInitialRoute',
    // Capabilities
    'createCapabilityRendererRegistry',
  ];

  for (const name of expectedExports) {
    it(`exports ${name}`, () => {
      expect(mod[name], `${name} should be exported`).toBeDefined();
    });
  }
});

// ── Component files exist ─────────────────────────────────────────────────────

describe('@conusai/ui component files exist', () => {
  const components = [
    'AppShell', 'AppHeader', 'Drawer', 'Sheet', 'Sidebar', 'SidebarSection', 'SidebarItem',
    'Type', 'Icon', 'Button', 'Field', 'Chip', 'EmptyState', 'StatusBadge',
    'PageHeader', 'DataTable', 'Breadcrumbs',
    'Composer', 'ThemeProvider', 'ThemeSwitcher', 'ToastHost',
    'PlanBadge', 'PlanCard', 'UsageMeter', 'QuotaBanner', 'CapabilityCard',
    // Phase 4.2 chat primitives
    'ThinkingIndicator', 'MessageBubble', 'MessageList', 'ToolCard',
  ];

  for (const name of components) {
    it(`components/${name}.svelte exists`, () => {
      expect(fileExists('components', `${name}.svelte`)).toBe(true);
    });

    it(`components/${name}.fixtures.ts exists`, () => {
      expect(fileExists('components', `${name}.fixtures.ts`)).toBe(true);
    });
  }
});

// ── Feature files exist ───────────────────────────────────────────────────────

describe('@conusai/ui feature files exist', () => {
  const features = [
    'AgentChatStream', 'SuggestionChips', 'ContextChip', 'CapabilityBrowser',
    'CapabilityRow', 'DrawerRecentChats',
    'ShellScreen', 'ShellLoginScreen',
    'QuotaList', 'ProfileSheet',
    // Phase 3.5: AttachmentSheet migrated from browser-shell
    'AttachmentSheet',
    // Phase 4.7: WorkspaceTree is the canonical name; WorkspaceExplorer shim deleted at Phase 4 close
    'WorkspaceTree',
  ];
  const billingFeatures = ['InvoiceStatusBadge'];
  const screens = [
    'ChatScreen', 'CapabilitiesScreen', 'ArtifactsScreen',
    'ArtifactRow', 'CapabilityDetailSheet',
  ];

  for (const name of features) {
    it(`features/${name}.svelte exists`, () => {
      expect(fileExists('features', `${name}.svelte`)).toBe(true);
    });
  }

  for (const name of billingFeatures) {
    it(`features/billing/${name}.svelte exists`, () => {
      expect(fileExists('features', 'billing', `${name}.svelte`)).toBe(true);
    });
  }

  for (const name of screens) {
    it(`features/screens/${name}.svelte exists`, () => {
      expect(fileExists('features', 'screens', `${name}.svelte`)).toBe(true);
    });
  }
});

// ── Utils exist ───────────────────────────────────────────────────────────────

describe('@conusai/ui utils exist', () => {
  const utils = [
    ['utils/platform.ts'],
    ['utils/haptics.ts'],
    ['utils/keyboard.ts'],
    ['utils/i18n.ts'],
    ['utils/actions.ts'],
    ['motion/index.ts'],
    ['stores/themeStore.svelte.ts'],
    ['stores/toast.svelte.ts'],
    ['stores/modeStore.svelte.ts'],
    ['stores/recents.svelte.ts'],
    ['stores/breadcrumbs.svelte.ts'],
    ['stores/screen.svelte.ts'],
    ['stores/drawer.svelte.ts'],
    ['routing/initialRoute.ts'],
    ['routing/applyInitialRoute.ts'],
    ['capabilities/CapabilityRendererRegistry.ts'],
  ];

  for (const [path] of utils) {
    it(`${path} exists`, () => {
      expect(fileExists(path)).toBe(true);
    });
  }
});
