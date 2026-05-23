import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'EmptyState',
  note: 'Zero-data illustration + CTA. kind maps to a hairline SVG glyph. Error/permission-denied kinds use danger color.',
  cases: [
    { label: 'no-chats',          props: { kind: 'no-chats',          title: 'No conversations yet',    body: 'Start a new chat to get help from your AI capabilities.',          actionLabel: 'New chat',         action: () => {} } },
    { label: 'no-artifacts',      props: { kind: 'no-artifacts',      title: 'No artifacts',            body: 'Files and outputs from your AI runs will appear here.',            actionLabel: 'Run a capability', action: () => {} } },
    { label: 'no-capabilities',   props: { kind: 'no-capabilities',   title: 'No capabilities found',   body: 'Try a different search term or browse all categories.',            actionLabel: 'Browse all',       action: () => {} } },
    { label: 'no-invoices',       props: { kind: 'no-invoices',       title: 'No invoices',             body: 'Your billing history will appear here once you upgrade.' } },
    { label: 'error',             props: { kind: 'error',             title: 'Something went wrong',    body: 'Try refreshing the page. If the problem persists, contact support.', actionLabel: 'Refresh',         action: () => {} } },
    { label: 'permission-denied', props: { kind: 'permission-denied', title: 'Access denied',           body: 'You don\'t have permission to view this page.',                    actionLabel: 'Go back',          action: () => {} } },
    { label: 'generic',           props: { kind: 'generic',           title: 'Nothing here yet',        body: 'Content will appear once available.' } },
    { label: 'compact no-chats',  props: { kind: 'no-chats',          title: 'No chats',                compact: true } },
    { label: 'with secondary',    props: { kind: 'error',             title: 'Request failed',          body: 'Could not load the resource.',  actionLabel: 'Retry', action: () => {}, secondaryLabel: 'Go home', secondaryAction: () => {} } },
  ],
};
export default fixtures;
