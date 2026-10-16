<svelte:options runes={true} />
<script lang="ts">
  /**
   * ShellLoginScreen — sign-in screen for the native shell.
   *
   * Pure presentational: one "Sign in" action that the app wires to its OIDC
   * flow (system browser + loopback PKCE in the Tauri shell). No credentials
   * are typed here and nothing is stored by this component.
   *
   * Props
   *   onSignIn  — called when the user asks to sign in
   *   busy      — true while the browser sign-in is in progress
   *   error     — message from the last failed attempt
   *   logoSrc   — optional logo image URL shown in the brand row
   *   company   — brand name shown next to the logo (default "ConusAI")
   *   title     — headline text (default "Enter the workshop.")
   *   subtitle  — body copy under headline
   */

  let {
    onSignIn,
    busy      = false,
    error     = null,
    logoSrc,
    company   = 'ConusAI',
    title     = 'Enter the workshop.',
    subtitle  = 'An agent platform built for operators who build with intent.',
  }: {
    onSignIn:  () => void;
    busy?:     boolean;
    error?:    string | null;
    logoSrc?:  string;
    company?:  string;
    title?:    string;
    subtitle?: string;
  } = $props();
</script>

<div class="login-screen">
  <div class="login-card" role="main">
    <!-- Brand -->
    <div class="login-brand">
      {#if logoSrc}
        <img class="brand-logo" src={logoSrc} alt={company} />
      {/if}
      <span class="brand-name">{company}</span>
    </div>

    <!-- Copy -->
    <div class="login-copy">
      <h1 class="login-title">{title}</h1>
      <p class="login-sub">{subtitle}</p>
    </div>

    {#if error}<p class="login-error" role="alert">{error}</p>{/if}

    <button class="begin-btn" type="button" onclick={onSignIn} disabled={busy} aria-busy={busy}>
      {busy ? 'Continue in your browser…' : 'Sign in'}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
           stroke-linecap="round" stroke-linejoin="round" width="18" height="18"
           aria-hidden="true">
        <path d="M5 12h14M12 5l7 7-7 7"/>
      </svg>
    </button>
  </div>
</div>

<style>
  /* ── Layout ── */
  .login-screen {
    min-height:      100dvh;
    display:         flex;
    align-items:     center;
    justify-content: center;
    background:      var(--color-bg);
    padding:         var(--space-4);
  }

  .login-card {
    width:          100%;
    max-width:      440px;
    display:        flex;
    flex-direction: column;
    gap:            var(--space-5);
  }

  /* ── Brand ── */
  .login-brand {
    display:     flex;
    align-items: center;
    gap:         var(--space-2);
  }

  .brand-logo {
    height: 28px;
    width:  auto;
    display: block;
  }

  .brand-name {
    font-family: var(--font-family-sans);
    font-size:   var(--font-size-h2);
    font-weight: 700;
    color:       var(--color-fg);
  }

  /* ── Copy ── */
  .login-copy { display: flex; flex-direction: column; }

  .login-title {
    font-family:    var(--font-family-sans);
    font-size:      32px;
    font-weight:    700;
    letter-spacing: -1px;
    line-height:    1.1;
    color:          var(--color-fg);
    margin:         0;
  }

  .login-sub {
    font-family: var(--font-family-sans);
    font-size:   var(--font-size-body);
    color:       var(--color-fg-muted);
    margin:      var(--space-2) 0 0;
    line-height: 1.5;
  }

  .login-error {
    font-family: var(--font-family-sans);
    font-size:   var(--font-size-meta);
    color:       var(--color-danger);
    margin:      0;
  }

  /* ── Sign-in button ── */
  .begin-btn {
    display:         flex;
    align-items:     center;
    justify-content: center;
    gap:             var(--space-2);
    height:          52px;
    background:      var(--color-accent);
    color:           var(--color-on-accent);
    border:          none;
    border-radius:   var(--radius-md);
    font-family:     var(--font-family-sans);
    font-size:       17px;
    font-weight:     600;
    cursor:          pointer;
    transition:      background var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }

  .begin-btn:hover:not(:disabled) { background: var(--color-accent-hover); }
  .begin-btn:disabled             { cursor: progress; opacity: 0.7; }
  .begin-btn:focus-visible        { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset); }

  @media (prefers-reduced-motion: reduce) {
    .begin-btn { transition: none; }
  }
</style>
