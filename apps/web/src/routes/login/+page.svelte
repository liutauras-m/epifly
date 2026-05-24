<svelte:options runes={true} />
<script lang="ts">
  /**
   * Login page — refactored to use shadcn primitives for the form, branded poster kept.
   */
  import type { PageData, ActionData } from './$types.js';
  import { AlertCircle } from '@lucide/svelte';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Label } from '$lib/components/ui/label/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import * as Alert from '$lib/components/ui/alert/index.js';
  import * as RadioGroup from '$lib/components/ui/radio-group/index.js';
  import logoDark from '@conusai/ui/assets/images/conusai-logo-darkmode.png';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let nameValue = $state(form?.name ?? 'John Smith');
  let planValue = $state('enterprise');

  const plans = ['free', 'pro', 'enterprise'] as const;
</script>

<svelte:head><title>Enter · ConusAI</title></svelte:head>

<div class="login-layout">

  <!-- ── Left: Poster (branded, kept) ─────────────────────────────── -->
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

  <!-- ── Right: Form (shadcn primitives) ──────────────────────────── -->
  <section class="flex-1 flex items-center justify-center px-6 py-10 overflow-y-auto">
    <form method="POST" aria-label="Sign in" class="w-full max-w-sm flex flex-col gap-6">

      <header class="flex flex-col gap-1">
        <p class="font-mono text-xs uppercase tracking-wider text-muted-foreground">
          {data.greeting} · ConusAI workshop
        </p>
        <h1 class="text-3xl font-semibold tracking-tight text-foreground">
          Enter the workshop.
        </h1>
      </header>

      <div class="flex flex-col gap-5">
        <!-- Operator name -->
        <div class="flex flex-col gap-2">
          <Label for="name">Operator name</Label>
          <Input
            id="name"
            name="name"
            type="text"
            bind:value={nameValue}
            placeholder="e.g. John Smith"
            required
            autocomplete="off"
            aria-invalid={form?.error && !form?.name ? 'true' : undefined}
          />
        </div>

        <!-- Plan tier -->
        <div class="flex flex-col gap-2">
          <Label>Plan tier</Label>
          <RadioGroup.Root bind:value={planValue} name="plan" class="grid grid-cols-3 gap-2">
            {#each plans as p}
              <Label
                for={`plan-${p}`}
                class="flex items-center gap-2 px-3 py-2 rounded-md border border-input text-sm text-muted-foreground cursor-pointer transition-colors hover:bg-muted has-data-[state=checked]:border-primary has-data-[state=checked]:bg-primary/5 has-data-[state=checked]:text-primary capitalize"
              >
                <RadioGroup.Item id={`plan-${p}`} value={p} />
                {p}
              </Label>
            {/each}
          </RadioGroup.Root>
        </div>
      </div>

      {#if form?.error}
        <Alert.Root variant="destructive">
          <AlertCircle class="size-4" />
          <Alert.Description>{form.error}</Alert.Description>
        </Alert.Root>
      {/if}

      <Button type="submit" size="lg" class="w-full">Begin</Button>

    </form>
  </section>
</div>

<style>
  /* ── Two-column layout (branded poster, not shadcn's role) ───────────────── */
  .login-layout {
    display:        flex;
    min-height:     100dvh;
    background:     var(--color-bg);
    container-type: inline-size;
    container-name: login-layout;
  }

  /* ── Poster (left) ───────────────────────────────────────────────────────── */
  .login-poster {
    --poster-gradient:     linear-gradient(135deg, var(--ember, #FF6200) 0%, color-mix(in srgb, var(--ember, #FF6200) 85%, #000) 60%, #111111 100%);
    --poster-em:           oklch(97% 0 0 / 0.92);
    --poster-hi:           oklch(80% 0.15 50 / 0.9);
    --poster-meta-color:   oklch(97% 0 0 / 0.5);
    --poster-tagline-size: clamp(var(--font-size-h2, 20px), 2.2vw, var(--font-size-display, 28px));

    display:    flex;
    flex:       0 0 45%;
    background: var(--poster-gradient);
    position:   relative;
    overflow:   hidden;
  }

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
    margin:         0;
    font-size:      var(--poster-tagline-size);
    font-weight:    500;
    line-height:    1.4;
    letter-spacing: -0.02em;
    color:          var(--poster-em);
  }
  .poster-tagline em {
    font-style:  normal;
    color:       var(--poster-hi);
    font-weight: 620;
  }

  .poster-meta {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-1);
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-meta);
    color:          var(--poster-meta-color);
    letter-spacing: 0.04em;
  }

  /* Mobile: collapse poster to top strip */
  @container login-layout (width < 1024px) {
    .login-layout    { flex-direction: column; }
    .login-poster    { flex: 0 0 30vh; min-height: 180px; }
    .poster-inner    { padding: var(--space-5) var(--space-5); flex-direction: row; align-items: center; flex-wrap: wrap; gap: var(--space-4); }
    .poster-tagline  { display: none; }
    .poster-meta     { display: none; }
  }
</style>
