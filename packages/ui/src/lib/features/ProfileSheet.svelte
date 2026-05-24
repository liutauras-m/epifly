<svelte:options runes={true} />
<script lang="ts">
  /**
   * ProfileSheet — user profile bottom-sheet for the shell (Phase 3.2).
   *
   * Shows avatar, name, plan, theme toggle, billing link, and sign-out.
   * Consumes the canonical <Sheet> primitive with semantic tokens.
   *
   * Usage:
   *   <ProfileSheet
   *     open={profileOpen}
   *     name={user.name}
   *     plan={user.plan}
   *     onclose={() => profileOpen = false}
   *     onLogout={handleLogout}
   *   />
   */
  import Sheet from '../components/Sheet.svelte';
  import { startViewTransition } from '../motion/index.js';

  let {
    open,
    name,
    plan,
    version = '',
    onclose,
    onLogout,
  }: {
    open: boolean;
    name: string;
    plan: string;
    /** Optional app version string displayed at the bottom, e.g. "0.4.0". */
    version?: string;
    onclose: () => void;
    onLogout: () => void;
  } = $props();

  let currentTheme = $state(
    typeof document !== 'undefined'
      ? (document.documentElement.dataset.theme ?? 'paper')
      : 'paper'
  );

  function toggleTheme() {
    // [feedback] Theme toggle — visual confirmation of change
    startViewTransition(() => {
      currentTheme = currentTheme === 'paper' ? 'forge' : 'paper';
      document.documentElement.dataset.theme = currentTheme;
      localStorage.setItem('conusai-theme', currentTheme);
    });
  }

  function initials(n: string) {
    return n.split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase();
  }
</script>

<Sheet {open} {onclose} label="Profile">
  {#snippet children()}
    <div class="profile-content">

      <!-- Avatar + name row -->
      <div class="profile-avatar-row">
        <div class="big-avatar" aria-hidden="true">{initials(name)}</div>
        <div class="profile-text">
          <div class="profile-name">{name}</div>
          <div class="profile-plan">{plan}</div>
        </div>
      </div>

      <div class="divider" role="separator" aria-hidden="true"></div>

      <!-- Theme toggle -->
      <button class="action-row" onclick={toggleTheme}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
             stroke-linecap="round" stroke-linejoin="round" width="20" height="20"
             aria-hidden="true">
          {#if currentTheme === 'paper'}
            <path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/>
          {:else}
            <circle cx="12" cy="12" r="5"/>
            <line x1="12" y1="1" x2="12" y2="3"/>
            <line x1="12" y1="21" x2="12" y2="23"/>
            <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/>
            <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
            <line x1="1" y1="12" x2="3" y2="12"/>
            <line x1="21" y1="12" x2="23" y2="12"/>
            <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/>
            <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
          {/if}
        </svg>
        <span>{currentTheme === 'paper' ? 'Switch to dark theme' : 'Switch to light theme'}</span>
      </button>

      <div class="divider" role="separator" aria-hidden="true"></div>

      <!-- Billing & Usage -->
      <a
        class="action-row"
        href="/account/billing"
        target="_blank"
        rel="noopener noreferrer"
        onclick={onclose}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
             stroke-linecap="round" stroke-linejoin="round" width="20" height="20"
             aria-hidden="true">
          <rect x="1" y="4" width="22" height="16" rx="2" ry="2"/>
          <line x1="1" y1="10" x2="23" y2="10"/>
        </svg>
        <span>Billing &amp; Usage</span>
      </a>

      <div class="divider" role="separator" aria-hidden="true"></div>

      <!-- Sign out -->
      <button
        class="action-row action-danger"
        onclick={() => { onLogout(); onclose(); }}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
             stroke-linecap="round" stroke-linejoin="round" width="20" height="20"
             aria-hidden="true">
          <path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/>
          <polyline points="16 17 21 12 16 7"/>
          <line x1="21" y1="12" x2="9" y2="12"/>
        </svg>
        <span>Sign out</span>
      </button>

      {#if version}
        <div class="version">v{version} · ConusAI</div>
      {/if}

    </div>
  {/snippet}
</Sheet>

<style>
  .profile-content {
    display:        flex;
    flex-direction: column;
    /* No padding-bottom needed — Sheet handles safe-area */
  }

  /* ── Avatar row ──────────────────────────────────────────────────────────── */
  .profile-avatar-row {
    display:     flex;
    align-items: center;
    gap:         var(--space-4);
    padding:     var(--space-4) var(--space-5);
  }

  .big-avatar {
    width:           56px;
    height:          56px;
    border-radius:   var(--radius-full);
    background:      var(--color-accent-soft);
    color:           var(--color-accent);
    font-family:     var(--font-family-sans);
    font-size:       20px;
    font-weight:     600;
    display:         flex;
    align-items:     center;
    justify-content: center;
    flex-shrink:     0;
    user-select:     none;
    border:          1px solid var(--color-border);
  }

  .profile-text {
    display:        flex;
    flex-direction: column;
    gap:            3px;
    overflow:       hidden;
  }

  .profile-name {
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-body);
    font-weight:    580;
    color:          var(--color-fg);
    overflow:       hidden;
    text-overflow:  ellipsis;
    white-space:    nowrap;
  }

  .profile-plan {
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    color:          var(--color-fg-subtle);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* ── Divider ─────────────────────────────────────────────────────────────── */
  .divider {
    height:     1px;
    background: var(--color-border);
    margin:     0 var(--space-5);
  }

  /* ── Action rows ─────────────────────────────────────────────────────────── */
  .action-row {
    display:         flex;
    align-items:     center;
    gap:             var(--space-3);
    height:          var(--hit, 56px);
    padding:         0 var(--space-5);
    border:          none;
    background:      none;
    font-family:     var(--font-family-sans);
    font-size:       var(--font-size-body);
    color:           var(--color-fg);
    cursor:          pointer;
    width:           100%;
    text-align:      left;
    text-decoration: none;
    transition:      background var(--duration-fast) var(--ease-standard);  /* [feedback] hover confirmation */
  }

  .action-row:hover {
    background: var(--color-bg-hover);
  }

  .action-row:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .action-danger {
    color: var(--color-danger);
  }

  .action-danger:hover {
    background: var(--color-danger-soft);
  }

  /* ── Version ─────────────────────────────────────────────────────────────── */
  .version {
    padding:        var(--space-4) var(--space-5);
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    color:          var(--color-fg-subtle);
    text-align:     center;
    letter-spacing: 0.04em;
  }

  @media (prefers-reduced-motion: reduce) {
    .action-row { transition: none; }
  }
</style>
