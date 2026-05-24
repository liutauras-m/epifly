<svelte:options runes={true} />
<script lang="ts">
  /**
   * Usage page — refactored to shadcn-svelte primitives.
   * Card + Progress + Alert replace ~104 lines of custom CSS.
   */
  import { ArrowUpRight, Sparkles } from '@lucide/svelte';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb/index.js';
  import * as Card from '$lib/components/ui/card/index.js';
  import * as Alert from '$lib/components/ui/alert/index.js';
  import { Progress } from '$lib/components/ui/progress/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import type { PageData } from './$types.js';

  let { data }: { data: PageData } = $props();
  const { usage, subscription } = data;

  const planKey = $derived(subscription?.plan_key ?? 'free');

  const limits: Record<string, { turns: number | null; tokens: number | null }> = {
    free:       { turns: 50,   tokens: null },
    pro:        { turns: 500,  tokens: null },
    team:       { turns: 2000, tokens: null },
    enterprise: { turns: null, tokens: null },
  };

  const limit = $derived(limits[planKey] ?? limits.free);
  const turnsPct = $derived(
    limit.turns ? Math.min(100, (usage.agent_turns / limit.turns) * 100) : 0
  );

  function fmt(n: number): string { return n.toLocaleString(); }
</script>

<svelte:head><title>Usage — ConusAI</title></svelte:head>

<div class="mx-auto max-w-2xl px-4 py-10 sm:py-14 flex flex-col gap-6">

  <!-- ── Breadcrumb + heading ─────────────────────────────────────── -->
  <div class="flex flex-col gap-4">
    <Breadcrumb.Root>
      <Breadcrumb.List>
        <Breadcrumb.Item>
          <Breadcrumb.Link href="/account">Account</Breadcrumb.Link>
        </Breadcrumb.Item>
        <Breadcrumb.Separator />
        <Breadcrumb.Item>
          <Breadcrumb.Page>Usage</Breadcrumb.Page>
        </Breadcrumb.Item>
      </Breadcrumb.List>
    </Breadcrumb.Root>

    <header class="flex flex-col gap-1">
      <span class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Usage</span>
      <h1 class="text-3xl font-semibold tracking-tight text-foreground">Usage</h1>
      <p class="text-sm text-muted-foreground">Today (UTC)</p>
    </header>
  </div>

  <!-- ── Meter cards ──────────────────────────────────────────────── -->
  <div class="flex flex-col gap-3">

    <!-- Agent Turns -->
    <Card.Root>
      <Card.Header>
        <div class="flex items-baseline justify-between gap-3">
          <Card.Title class="text-base">Agent Turns</Card.Title>
          <span class="text-lg font-bold tracking-tight text-foreground tabular-nums">
            {fmt(usage.agent_turns)}{limit.turns ? ` / ${fmt(limit.turns)}` : ''}
          </span>
        </div>
      </Card.Header>
      <Card.Content class="flex flex-col gap-3">
        {#if limit.turns}
          <Progress value={turnsPct} />
          <p class="font-mono text-xs text-muted-foreground tracking-wide">
            {fmt(limit.turns - usage.agent_turns)} remaining today
          </p>
        {:else}
          <p class="font-mono text-xs text-emerald-600 tracking-wide">Unlimited on your plan</p>
        {/if}
      </Card.Content>
    </Card.Root>

    <!-- Tokens -->
    <Card.Root>
      <Card.Header>
        <div class="flex items-baseline justify-between gap-3">
          <Card.Title class="text-base">Tokens Used</Card.Title>
          <span class="text-lg font-bold tracking-tight text-foreground tabular-nums">{fmt(usage.tokens)}</span>
        </div>
      </Card.Header>
      <Card.Content>
        <p class="font-mono text-xs text-muted-foreground tracking-wide">
          Billed as usage — see invoices for breakdown
        </p>
      </Card.Content>
    </Card.Root>

    <!-- Storage -->
    <Card.Root>
      <Card.Header>
        <div class="flex items-baseline justify-between gap-3">
          <Card.Title class="text-base">Storage</Card.Title>
          <span class="text-lg font-bold tracking-tight text-foreground tabular-nums">{usage.storage_gb.toFixed(2)} GB</span>
        </div>
      </Card.Header>
      <Card.Content>
        <p class="font-mono text-xs text-muted-foreground tracking-wide">
          Workspace files and artifacts
        </p>
      </Card.Content>
    </Card.Root>

  </div>

  <!-- ── Upgrade banner (free tier only) ──────────────────────────── -->
  {#if planKey === 'free'}
    <Alert.Root class="bg-primary/5 border-primary/30">
      <Sparkles class="size-4 text-primary" />
      <Alert.Title class="text-foreground">Upgrade for more capacity</Alert.Title>
      <Alert.Description class="text-muted-foreground">
        You're on the Free plan. Upgrade for more turns, tokens, and storage.
      </Alert.Description>
      <Alert.Action>
        <Button href="/account/billing" size="sm">
          Upgrade
          <ArrowUpRight class="size-3.5" />
        </Button>
      </Alert.Action>
    </Alert.Root>
  {/if}

</div>
