/**
 * i18n.ts — internationalisation skeleton (Phase 7).
 *
 * Provides a minimal translation layer that:
 *   1. Accepts a locale string and a flat key→value dictionary.
 *   2. Returns a `t(key, vars?)` function for string interpolation.
 *   3. Falls back to the raw key when a translation is missing.
 *   4. Supports simple `{name}` variable substitution.
 *
 * This is NOT a full i18n library. It is the minimum surface to:
 *   - Let all user-visible strings in @conusai/ui flow through a single point.
 *   - Allow downstream apps to inject locale dictionaries at startup.
 *   - Enable future swapout to Fluent/FormatJS without component changes.
 *
 * Usage:
 *   import { createI18n, setI18n, t } from '@conusai/ui/utils/i18n.js';
 *
 *   // App bootstrap:
 *   setI18n(createI18n('en', enMessages));
 *
 *   // In components:
 *   {t('composer.placeholder')}        // → "Ask anything…"
 *   {t('greeting.morning', { name })}  // → "Good morning, Alice."
 *
 * Default locale is 'en'. Calling setI18n is optional — the default
 * translator returns the key unchanged so components remain functional
 * without i18n wiring.
 */

export type I18nMessages = Record<string, string>;

export interface I18nInstance {
  locale: string;
  t: (key: string, vars?: Record<string, string | number>) => string;
}

/**
 * Create an i18n instance from a locale and a flat message dictionary.
 */
export function createI18n(locale: string, messages: I18nMessages): I18nInstance {
  return {
    locale,
    t(key, vars) {
      let msg = messages[key];
      if (msg === undefined) {
        // Development: warn once per missing key
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        if (typeof (globalThis as any).process !== 'undefined' && (globalThis as any).process?.env?.NODE_ENV !== 'production') {
          // eslint-disable-next-line no-console
          console.warn(`[i18n] Missing key: "${key}" for locale "${locale}"`);
        }
        msg = key;
      }
      if (vars) {
        for (const [k, v] of Object.entries(vars)) {
          msg = msg.replaceAll(`{${k}}`, String(v));
        }
      }
      return msg;
    },
  };
}

// ── Module-level singleton ────────────────────────────────────────────────────

/** The active i18n instance. Defaults to identity (key → key). */
let _i18n: I18nInstance = {
  locale: 'en',
  t: (key) => key,
};

/** Replace the active i18n instance. Call once at app bootstrap. */
export function setI18n(instance: I18nInstance): void {
  _i18n = instance;
}

/** Get the active i18n instance. */
export function getI18n(): I18nInstance {
  return _i18n;
}

/**
 * Translate a key using the active i18n instance.
 * Safe to call before setI18n — returns the raw key.
 */
export function t(key: string, vars?: Record<string, string | number>): string {
  return _i18n.t(key, vars);
}

// ── Default English messages ──────────────────────────────────────────────────

/** Built-in English fallback dictionary for all @conusai/ui strings. */
export const enMessages: I18nMessages = {
  // Composer
  'composer.placeholder':      'Ask anything…',
  'composer.send':             'Send message',
  'composer.attach':           'Attach file',
  'composer.remove_attachment': 'Remove {name}',

  // Greetings
  'greeting.morning':   'Good morning, {name}.',
  'greeting.afternoon': 'Good afternoon, {name}.',
  'greeting.evening':   'Good evening, {name}.',
  'greeting.subtitle':  'How can I help you today?',

  // Navigation
  'nav.skip_to_main':     'Skip to main content',
  'nav.skip_to_composer': 'Skip to composer',
  'nav.open_navigation':  'Open navigation',
  'nav.close_navigation': 'Close navigation',
  'nav.go_back':          'Go back',
  'nav.new_chat':         'New conversation',
  'nav.account_settings': 'Open account settings',
  'nav.sign_out':         'Sign out',

  // Screens
  'screen.capabilities': 'Capabilities',
  'screen.artifacts':    'Artifacts',
  'screen.workshop':     'Workshop',

  // Empty states
  'empty.no_chats':       'No conversations yet',
  'empty.no_artifacts':   'No artifacts yet',
  'empty.no_capabilities':'No capabilities found',
  'empty.no_invoices':    'No invoices yet',
  'empty.error':          'Something went wrong',
  'empty.permission':     'Access denied',

  // Status
  'status.success': 'Success',
  'status.warning': 'Warning',
  'status.danger':  'Error',
  'status.neutral': 'Neutral',
  'status.info':    'Info',

  // Tools
  'tool.running': 'Running',
  'tool.retry':   'Retry',

  // Toasts
  'toast.dismiss': 'Dismiss',

  // Theme
  'theme.system': 'System theme',
  'theme.light':  'Light theme',
  'theme.dark':   'Dark theme',
};
