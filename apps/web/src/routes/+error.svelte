<svelte:options runes={true} />
<script lang="ts">
  /**
   * +error.svelte — Phase 4.9
   * Route-level errors rendered via EmptyState primitive.
   * Zero local CSS — all styles in packages/ui.
   */
  import { page } from '$app/stores';
  import { EmptyState } from '@conusai/ui';
</script>

<svelte:head>
  <title>Error {$page.status} · ConusAI</title>
</svelte:head>

<div class="error-screen">
  <EmptyState
    kind={$page.status === 403 || $page.status === 401 ? 'permission-denied' : 'error'}
    title={$page.error?.message ?? 'Something went wrong'}
    body={$page.status === 404
      ? 'The page you\'re looking for doesn\'t exist.'
      : $page.status >= 500
        ? 'A server error occurred. Try refreshing, or contact support if it persists.'
        : 'An unexpected error occurred.'}
    actionLabel="Back to workshop"
    action={() => history.back()}
    secondaryLabel="Go home"
    secondaryAction={() => { window.location.href = '/'; }}
  />
</div>

<style>
  .error-screen {
    display:          flex;
    align-items:      center;
    justify-content:  center;
    min-height:       100dvh;
    padding:          var(--space-5);
    background:       var(--color-bg);
  }
</style>
