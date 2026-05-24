<svelte:options runes={true} />
<script lang="ts">
  /**
   * ShellLoginScreen — quick-access login form for the browser/Tauri shell (Phase 3.1).
   *
   * Pure presentational: renders the login form and dispatches submit via `onSubmit`.
   * Auth state, session cookie issuance, and localStorage live in the app layer.
   *
   * Props
   *   onSubmit   — called with `(name, plan)` when the form is submitted successfully
   *   nameError  — validation error message to show under the name field
   *   logoSrc    — optional logo image URL shown in the brand row
   *   company    — brand name shown next to the logo (default "ConusAI")
   *   title      — headline text (default "Enter the workshop.")
   *   subtitle   — body copy under headline
   */

  let {
    onSubmit,
    nameError = '',
    logoSrc,
    company   = 'ConusAI',
    title     = 'Enter the workshop.',
    subtitle  = 'An agent platform built for operators who build with intent.',
  }: {
    onSubmit:   (name: string, plan: string) => void;
    nameError?: string;
    logoSrc?:   string;
    company?:   string;
    title?:     string;
    subtitle?:  string;
  } = $props();

  let nameInput = $state('');
  let planInput = $state('enterprise');
  let localError = $state('');

  const error = $derived(nameError || localError);

  function handleSubmit() {
    const name = nameInput.trim();
    if (!name || name.length > 60) {
      localError = 'Name must be 1–60 characters.';
      return;
    }
    localError = '';
    onSubmit(name, planInput);
  }
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

    <!-- Form -->
    <form class="login-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
      <div class="field">
        <label class="field-label" for="shell-name-input">Your name</label>
        <input
          id="shell-name-input"
          class="field-input"
          class:error={!!error}
          type="text"
          bind:value={nameInput}
          placeholder="John Smith"
          maxlength="60"
          autocomplete="off"
          autocorrect="off"
          autocapitalize="words"
          spellcheck={false}
          required
        />
        {#if error}<p class="field-error" role="alert">{error}</p>{/if}
      </div>

      <fieldset class="plan-fieldset">
        <legend class="field-label">Plan tier</legend>
        <div class="plan-row">
          {#each [['free', 'Free'], ['pro', 'Pro'], ['enterprise', 'Enterprise']] as [val, label] (val)}
            <label class="plan-option" class:selected={planInput === val}>
              <input type="radio" name="shell-plan" value={val} bind:group={planInput} />
              <span class="plan-label">{label}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <button class="begin-btn" type="submit">
        Get started
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
             stroke-linecap="round" stroke-linejoin="round" width="18" height="18"
             aria-hidden="true">
          <path d="M5 12h14M12 5l7 7-7 7"/>
        </svg>
      </button>
    </form>
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

  /* ── Form ── */
  .login-form { display: flex; flex-direction: column; gap: var(--space-4); }

  .field { display: flex; flex-direction: column; gap: var(--space-1); }

  .field-label {
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    font-weight:    500;
    letter-spacing: 0.08em;
    color:          var(--color-fg-subtle);
    text-transform: uppercase;
  }

  .field-input {
    height:      48px;
    border:      1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding:     0 var(--space-4);
    background:  var(--color-bg-raised);
    color:       var(--color-fg);
    font-family: var(--font-family-sans);
    font-size:   16px;
    transition:  border-color var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }

  .field-input:focus        { outline: none; border-color: var(--color-accent); }
  .field-input.error        { border-color: var(--color-danger); }
  .field-input:focus-visible { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset); }

  .field-error {
    font-family: var(--font-family-sans);
    font-size:   var(--font-size-meta);
    color:       var(--color-danger);
    margin:      0;
  }

  /* ── Plan picker ── */
  .plan-fieldset {
    border:  none;
    padding: 0;
    margin:  0;
    display: flex;
    flex-direction: column;
    gap:     var(--space-2);
  }

  .plan-row { display: flex; gap: var(--space-2); }

  .plan-option {
    flex:            1;
    display:         flex;
    align-items:     center;
    justify-content: center;
    height:          var(--hit, 44px);
    border:          1px solid var(--color-border);
    border-radius:   var(--radius-md);
    cursor:          pointer;
    transition:
      border-color var(--duration-fast) var(--ease-standard),  /* [feedback] */
      background   var(--duration-fast) var(--ease-standard);
  }

  .plan-option.selected {
    border-color: var(--color-accent);
    background:   var(--color-accent-soft);
  }

  .plan-option input { display: none; }

  .plan-label {
    font-family: var(--font-family-sans);
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-muted);
  }

  .plan-option.selected .plan-label { color: var(--color-fg); font-weight: 600; }

  /* ── Submit button ── */
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

  .begin-btn:hover        { background: var(--color-accent-hover); }
  .begin-btn:focus-visible { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset); }

  @media (prefers-reduced-motion: reduce) {
    .plan-option, .begin-btn, .field-input { transition: none; }
  }
</style>
