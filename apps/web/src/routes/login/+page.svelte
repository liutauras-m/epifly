<svelte:options runes={true} />
<script lang="ts">
  /**
   * Login page — Phase 4.3
   * Two-column poster + form layout. Field + Button primitives.
   * Zero app-local styling for interactive elements.
   */
  import type { PageData, ActionData } from './$types';
  import { Field, Button } from '@conusai/ui';
  import logoDark from '@conusai/ui/assets/images/conusai-logo-darkmode.png';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  // Bind values so Field primitive gets them
  let nameValue  = $state(form?.name ?? 'John Smith');
  let planValue  = $state('enterprise');
</script>

<svelte:head>
  <title>Enter · ConusAI</title>
</svelte:head>

<div class="login-layout">

  <!-- ── Left: Poster ────────────────────────────────────────────── -->
  <aside class="login-poster" aria-hidden="true">
    <div class="poster-inner">
      <img src={logoDark} alt="" class="poster-logo" width="140" />
      <blockquote class="poster-tagline">
        An <em>agent workshop</em> for operators who build with intent.
      </blockquote>
      <footer class="poster-meta">
        <span>v0.4 · {new Date().getFullYear()}</span>
        <span>Forge · stream · inspect</span>
      </footer>
    </div>
  </aside>

  <!-- ── Right: Form ─────────────────────────────────────────────── -->
  <section class="login-form-wrap">
    <form class="login-form" method="POST" aria-label="Sign in">

      <header class="form-header">
        <p class="form-eyebrow">{data.greeting} · ConusAI workshop</p>
        <h1 class="form-title">Enter the workshop.</h1>
      </header>

      <div class="form-fields">
        <Field
          id="name"
          label="Operator name"
          type="text"
          bind:value={nameValue}
          placeholder="e.g. John Smith"
          required
          autocomplete="off"
          error={form?.error && !form?.name ? form.error : undefined}
        />

        <!-- Plan tier radio group -->
        <fieldset class="plan-fieldset">
          <legend class="plan-legend">Plan tier</legend>
          <div class="plan-options">
            <label class="plan-option">
              <input type="radio" name="plan" value="free" bind:group={planValue} />
              <span>Free</span>
            </label>
            <label class="plan-option">
              <input type="radio" name="plan" value="pro" bind:group={planValue} />
              <span>Pro</span>
            </label>
            <label class="plan-option">
              <input type="radio" name="plan" value="enterprise" bind:group={planValue} />
              <span>Enterprise</span>
            </label>
          </div>
        </fieldset>
      </div>

      {#if form?.error}
        <p class="form-error" role="alert">{form.error}</p>
      {/if}

      <Button
        type="submit"
        variant="primary"
        size="lg"
        fullWidth
        text="Begin"
      />

    </form>
  </section>
</div>

<style>
  /* ── Two-column layout ───────────────────────────────────────────────────── */
  .login-layout {
    display:    flex;
    min-height: 100dvh;
    background: var(--color-bg);
  }

  /* ── Poster (left) ───────────────────────────────────────────────────────── */
  .login-poster {
    display:    flex;
    flex:       0 0 45%;
    background: var(--poster-gradient, linear-gradient(135deg, #FF6200 0%, #E05500 60%, #111111 100%));
    position:   relative;
    overflow:   hidden;
  }

  /* Noise overlay */
  .login-poster::after {
    content:    '';
    position:   absolute;
    inset:      0;
    background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 200 200' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.04'/%3E%3C/svg%3E");
    background-size:   200px;
    pointer-events:    none;
  }

  .poster-inner {
    position:       relative;
    z-index:        1;
    display:        flex;
    flex-direction: column;
    justify-content: space-between;
    padding:        var(--space-8) var(--space-7);
    width:          100%;
  }

  .poster-logo {
    width:      120px;
    height:     auto;
    object-fit: contain;
  }

  .poster-tagline {
    margin:        0;
    font-size:     clamp(20px, 2.2vw, 28px);
    font-weight:   500;
    line-height:   1.4;
    letter-spacing: -0.02em;
    color:         var(--poster-em, rgba(255,255,255,0.92));
  }
  .poster-tagline em {
    font-style:  normal;
    color:       var(--poster-hi, rgba(255,150,80,0.9));
    font-weight: 620;
  }

  .poster-meta {
    display:         flex;
    flex-direction:  column;
    gap:             var(--space-1);
    font-family:     var(--font-family-mono);
    font-size:       var(--font-size-meta);
    color:           rgba(255, 255, 255, 0.5);
    letter-spacing:  0.04em;
  }

  /* Mobile: poster shrinks to 30vh header strip */
  @media (max-width: 1023px) {
    .login-layout    { flex-direction: column; }
    .login-poster    { flex: 0 0 30vh; min-height: 180px; }
    .poster-inner    { padding: var(--space-5) var(--space-5); flex-direction: row; align-items: center; flex-wrap: wrap; gap: var(--space-4); }
    .poster-tagline  { display: none; }
    .poster-meta     { display: none; }
  }

  /* ── Form (right) ────────────────────────────────────────────────────────── */
  .login-form-wrap {
    flex:            1;
    display:         flex;
    align-items:     center;
    justify-content: center;
    padding:         var(--space-7) var(--space-5);
    overflow-y:      auto;
  }

  .login-form {
    width:     100%;
    max-width: 400px;
    display:   flex;
    flex-direction: column;
    gap:       var(--space-5);
  }

  /* ── Form header ─────────────────────────────────────────────────────────── */
  .form-header {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-1);
  }

  .form-eyebrow {
    margin:         0;
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-meta);
    letter-spacing: 0.06em;
    color:          var(--color-fg-subtle);
    text-transform: uppercase;
  }

  .form-title {
    margin:         0;
    font-size:      var(--font-size-h1);   /* 28px */
    font-weight:    620;
    letter-spacing: -0.025em;
    color:          var(--color-fg);
  }

  /* ── Fields ──────────────────────────────────────────────────────────────── */
  .form-fields {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-4);
  }

  /* Plan fieldset */
  .plan-fieldset {
    border:  none;
    padding: 0;
    margin:  0;
  }

  .plan-legend {
    font-size:   var(--font-size-meta);
    font-weight: 500;
    color:       var(--color-fg-muted);
    margin-bottom: var(--space-2);
  }

  .plan-options {
    display: flex;
    gap:     var(--space-2);
    flex-wrap: wrap;
  }

  .plan-option {
    display:     flex;
    align-items: center;
    gap:         var(--space-1);
    padding:     var(--space-2) var(--space-3);
    border:      1px solid var(--color-border);
    border-radius: var(--radius-sm);
    cursor:      pointer;
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-muted);
    transition:  border-color var(--duration-fast), background var(--duration-fast);
  }

  .plan-option:has(input:checked) {
    border-color: var(--color-accent);
    color:        var(--color-accent);
    background:   var(--color-accent-soft);
  }

  .plan-option input {
    width:  14px;
    height: 14px;
    accent-color: var(--color-accent);
  }

  /* ── Error ───────────────────────────────────────────────────────────────── */
  .form-error {
    margin:     0;
    font-size:  var(--font-size-meta);
    color:      var(--color-danger);
    padding:    var(--space-2) var(--space-3);
    background: var(--color-danger-soft);
    border:     1px solid var(--color-danger);
    border-radius: var(--radius-xs);
  }
</style>
