<svelte:options runes={true} />
<script lang="ts">
  /**
   * Billing page — refactored to shadcn-svelte primitives.
   * Card + Badge + Alert + Breadcrumb replace ~189 lines of custom CSS.
   */
  import { enhance } from '$app/forms';
  import { Layers, Zap, Users, Building2, Check, ArrowUpRight, AlertCircle } from '@lucide/svelte';
  import { PlanBadge, StatusBadge } from '@conusai/ui';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb/index.js';
  import * as Card from '$lib/components/ui/card/index.js';
  import * as Alert from '$lib/components/ui/alert/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import type { ActionData, PageData } from './$types.js';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  const { plans, subscription } = data;

  const currentPlan = $derived(subscription?.plan_key ?? 'free');
  const isActive    = $derived(
    subscription?.status === 'active' || subscription?.status === 'trialing'
  );

  const subscriptionStatus = $derived((): import('@conusai/ui').StatusKind => {
    switch (subscription?.status) {
      case 'active':
      case 'trialing': return 'success';
      case 'past_due': return 'warning';
      case 'canceled': return 'danger';
      default:         return 'neutral';
    }
  });

  const planIcons: Record<string, typeof Zap> = {
    free: Layers, pro: Zap, team: Users, enterprise: Building2,
  };
</script>

<svelte:head><title>Billing — ConusAI</title></svelte:head>

<div class="mx-auto max-w-4xl px-4 py-10 sm:py-14 flex flex-col gap-8">

  <!-- ── Breadcrumb + heading ─────────────────────────────────────── -->
  <div class="flex flex-col gap-4">
    <Breadcrumb.Root>
      <Breadcrumb.List>
        <Breadcrumb.Item>
          <Breadcrumb.Link href="/account">Account</Breadcrumb.Link>
        </Breadcrumb.Item>
        <Breadcrumb.Separator />
        <Breadcrumb.Item>
          <Breadcrumb.Page>Billing</Breadcrumb.Page>
        </Breadcrumb.Item>
      </Breadcrumb.List>
    </Breadcrumb.Root>

    <header class="flex flex-col gap-1">
      <span class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Billing</span>
      <h1 class="text-3xl font-semibold tracking-tight text-foreground">Billing &amp; Plans</h1>
    </header>
  </div>

  <!-- ── Error banner ─────────────────────────────────────────────── -->
  {#if form?.error}
    <Alert.Root variant="destructive">
      <AlertCircle class="size-4" />
      <Alert.Description>{form.error}</Alert.Description>
    </Alert.Root>
  {/if}

  <!-- ── Current plan ─────────────────────────────────────────────── -->
  {#if subscription}
    <Card.Root>
      <Card.Header>
        <Card.Title class="text-base">Current Plan</Card.Title>
      </Card.Header>
      <Card.Content class="flex flex-col gap-4">
        <div class="flex items-center gap-3 flex-wrap">
          <PlanBadge tier={currentPlan} />
          <StatusBadge
            status={subscriptionStatus()}
            label={subscription.status.replace('_', ' ')}
          />
          {#if subscription.current_period_end}
            <span class="font-mono text-xs text-muted-foreground tracking-wide">
              Renews {new Date(subscription.current_period_end).toLocaleDateString()}
            </span>
          {/if}
        </div>
      </Card.Content>
      <Card.Footer class="gap-3 flex-wrap">
        <form method="POST" action="?/portal" use:enhance>
          <Button type="submit" variant="secondary" size="sm">Manage Billing</Button>
        </form>
        {#if isActive && currentPlan !== 'free'}
          <form method="POST" action="?/cancel" use:enhance>
            <Button type="submit" variant="destructive" size="sm">Cancel Subscription</Button>
          </form>
        {/if}
      </Card.Footer>
    </Card.Root>
  {/if}

  <!-- ── Plan cards ───────────────────────────────────────────────── -->
  <section aria-label="Available plans" class="flex flex-col gap-4">
    <h2 class="text-base font-semibold tracking-tight text-foreground">Available Plans</h2>
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
      {#each plans as plan (plan.key)}
        {@const isCurrent = plan.key === currentPlan}
        {@const PlanIcon  = planIcons[plan.key] ?? Layers}
        <Card.Root class={isCurrent ? 'border-primary ring-2 ring-primary/20' : ''}>
          <Card.Header>
            <div class="flex items-center gap-2 text-primary">
              <PlanIcon class="size-5" strokeWidth={1.5} />
            </div>
            <Card.Title>{plan.display_name}</Card.Title>
            <Card.Description class="text-2xl font-bold tracking-tight text-foreground">
              {#if plan.monthly_price_cents === 0}
                Free
              {:else}
                ${(plan.monthly_price_cents / 100).toFixed(0)}<span class="text-sm font-normal text-muted-foreground">/mo</span>
              {/if}
            </Card.Description>
          </Card.Header>
          <Card.Content class="flex-1">
            <ul class="flex flex-col gap-1.5 text-sm text-muted-foreground">
              <li class="flex items-center gap-2">
                <Check class="size-3.5 text-emerald-600 shrink-0" strokeWidth={2.5} />
                {plan.max_turns_per_day ? `${plan.max_turns_per_day.toLocaleString()} agent turns/day` : 'Unlimited agent turns'}
              </li>
              <li class="flex items-center gap-2">
                <Check class="size-3.5 text-emerald-600 shrink-0" strokeWidth={2.5} />
                {plan.max_storage_gb ? `${plan.max_storage_gb} GB storage` : 'Unlimited storage'}
              </li>
              <li class="flex items-center gap-2">
                <Check class="size-3.5 text-emerald-600 shrink-0" strokeWidth={2.5} />
                {plan.max_tokens.toLocaleString()} tokens/request
              </li>
              <li class="flex items-center gap-2">
                <Check class="size-3.5 text-emerald-600 shrink-0" strokeWidth={2.5} />
                {plan.rate_limit_rpm} requests/min
              </li>
            </ul>
          </Card.Content>
          <Card.Footer>
            {#if !isCurrent && plan.key !== 'enterprise'}
              <form method="POST" action="?/upgrade" use:enhance class="w-full">
                <input type="hidden" name="plan_key" value={plan.key} />
                <Button type="submit" size="sm" class="w-full">
                  {plan.monthly_price_cents > 0 ? 'Upgrade' : 'Downgrade'}
                  <ArrowUpRight class="size-3.5" />
                </Button>
              </form>
            {:else if isCurrent}
              <div class="w-full text-center font-mono text-xs uppercase tracking-widest text-primary font-semibold">
                Current Plan
              </div>
            {:else}
              <Button href="mailto:sales@conusai.com" variant="outline" size="sm" class="w-full">
                Contact Sales
              </Button>
            {/if}
          </Card.Footer>
        </Card.Root>
      {/each}
    </div>
  </section>

  <!-- ── Invoices ─────────────────────────────────────────────────── -->
  <Card.Root>
    <Card.Header>
      <Card.Title class="text-base">Invoices</Card.Title>
      <Card.Description>View and download invoices from the billing portal.</Card.Description>
    </Card.Header>
    <Card.Footer>
      <form method="POST" action="?/portal" use:enhance>
        <Button type="submit" variant="ghost" size="sm">Open billing portal</Button>
      </form>
    </Card.Footer>
  </Card.Root>

</div>
