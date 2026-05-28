<script lang="ts">
  import { AiToolProgress } from "@epifly/ui";
</script>

<svelte:head>
  <title>Privacy · Epifly</title>
</svelte:head>

<main class="legal-page">
  <section class="legal-page__card" aria-labelledby="privacy-title">
    <a class="legal-page__brand" href="/login" aria-label="Back to Epifly login">
      <AiToolProgress state="idle" size="sm" showLabel={false} />
      <span>Epifly</span>
    </a>

    <p class="legal-page__eyebrow">Privacy</p>
    <h1 id="privacy-title">Privacy policy</h1>
    <p class="legal-page__lead">
      This page is a placeholder for the production privacy policy. Epifly should explain clearly what account, prompt, file, and workspace data is collected before public launch.
    </p>

    <div class="legal-page__body">
      <p>Use the minimum data needed to provide the product experience, secure the account, and keep workspace context available across sessions.</p>
      <p>Production privacy terms should cover retention, deletion, model-provider processing, support access, security controls, and user rights in the jurisdictions where Epifly operates.</p>
    </div>
  </section>
</main>

<style>
  .legal-page {
    min-height: 100svh;
    display: grid;
    place-items: center;
    padding: max(1rem, var(--safe-top)) max(1rem, var(--safe-right)) max(1rem, var(--safe-bottom)) max(1rem, var(--safe-left));
    background:
      radial-gradient(circle at 12% 8%, color-mix(in oklch, var(--epifly-tool-cyan) 8%, transparent), transparent 30%),
      radial-gradient(circle at 86% 16%, color-mix(in oklch, var(--epifly-logo-orange) 11%, transparent), transparent 30%),
      linear-gradient(180deg, color-mix(in oklch, var(--background) 98%, var(--epifly-logo-orange) 2%), var(--background));
  }

  .legal-page__card {
    width: min(44rem, 100%);
    border: 1px solid color-mix(in oklch, var(--border) 84%, transparent);
    border-radius: 28px;
    background: color-mix(in oklch, var(--card) 88%, transparent);
    box-shadow: 0 1.4rem 4rem color-mix(in oklch, var(--foreground) 10%, transparent);
    color: var(--foreground);
    padding: clamp(1.5rem, 5vw, 3rem);
  }

  .legal-page__brand {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    color: inherit;
    font-weight: 700;
    letter-spacing: -0.04em;
    text-decoration: none;
  }

  .legal-page__eyebrow {
    margin: 2.5rem 0 0.8rem;
    color: var(--epifly-logo-orange);
    font-size: 0.74rem;
    font-weight: 800;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .legal-page h1 {
    margin: 0;
    font-size: clamp(2.4rem, 8vw, 4.25rem);
    letter-spacing: -0.06em;
    line-height: 0.96;
  }

  .legal-page__lead,
  .legal-page__body {
    color: var(--muted-foreground);
    font-size: clamp(1rem, 2vw, 1.08rem);
    line-height: 1.65;
  }

  .legal-page__lead {
    margin: 1.2rem 0 0;
  }

  .legal-page__body {
    display: grid;
    gap: 0.85rem;
    margin-top: 1.5rem;
  }

  .legal-page__body p {
    margin: 0;
  }
</style>