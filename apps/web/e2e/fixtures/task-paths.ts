/**
 * Canonical top-5 task paths (Phase 1.5).
 *
 * Single source of truth imported by:
 *   - e2e/visual/visual.spec.ts  (screenshot baselines)
 *   - e2e/motion-budget.spec.ts  (Phase 6 — animation duration audit)
 *   - e2e/a11y.spec.ts           (Phase 7 — VoiceOver / axe walks)
 *   - e2e/reduced-motion.spec.ts (Phase 7 — prefers-reduced-motion gate)
 */

export interface TaskPath {
  id: string;
  label: string;
  /** Starting URL (relative, no leading protocol/host). */
  startUrl: string;
  /** Description of the steps involved (for test labelling). */
  steps: string[];
}

export const TASK_PATHS: TaskPath[] = [
  {
    id: 'cold-start',
    label: 'Cold start to greeting',
    startUrl: '/login',
    steps: ['navigate /login', 'authenticate', 'land on /'],
  },
  {
    id: 'send-message',
    label: 'Send message',
    startUrl: '/',
    steps: ['focus composer', 'type prompt', 'submit', 'first token rendered'],
  },
  {
    id: 'capability-detail',
    label: 'Open capability detail',
    startUrl: '/',
    steps: ['open capabilities browser', 'select a capability row', 'detail sheet visible'],
  },
  {
    id: 'artifact-preview',
    label: 'Open artifact preview',
    startUrl: '/',
    steps: ['navigate to artifacts', 'click artifact row', 'preview sheet visible'],
  },
  {
    id: 'change-theme',
    label: 'Change theme',
    startUrl: '/',
    steps: ['open account menu', 'toggle theme', 'repaint settled'],
  },
];

/** Convenience: the start URLs used by visual regression tests. */
export const VISUAL_ROUTES = [
  { path: '/login',           label: 'login' },
  { path: '/',                label: 'home' },
  { path: '/account',         label: 'account' },
  { path: '/account/billing', label: 'billing' },
  { path: '/account/usage',   label: 'usage' },
] as const;
