<svelte:options runes={true} />
<script lang="ts">
  /**
   * Account page — refactored to shadcn-svelte primitives.
   * Card + Avatar + Badge + Item replace the previous 166 lines of custom CSS.
   */
  import { CreditCard, BarChart3, LogOut, ChevronRight } from '@lucide/svelte';
  import { PlanBadge } from '@conusai/ui';
  import * as Avatar from '$lib/components/ui/avatar/index.js';
  import * as Card from '$lib/components/ui/card/index.js';
  import * as Item from '$lib/components/ui/item/index.js';
  import { Badge } from '$lib/components/ui/badge/index.js';
  import type { PageData } from './$types.js';

  let { data }: { data: PageData } = $props();

  const { user, subscription, authProvider } = data;

  const planLabel   = $derived(subscription?.plan_key ?? user?.plan ?? 'free');
  const statusLabel = $derived(subscription?.status ?? 'active');
  const showStatus  = $derived(statusLabel !== 'active' && statusLabel !== 'trialing');
  const logoutHref  = $derived(authProvider === 'zitadel' ? '/auth/logout' : '/logout');
  const initial     = $derived((user?.name ?? '?')[0].toUpperCase());
</script>

<svelte:head><title>Account — ConusAI</title></svelte:head>

<div class="mx-auto max-w-xl px-4 py-10 sm:py-14 flex flex-col gap-6">

  <!-- ── Page heading ───────────────────────────────────────────── -->
  <header class="flex flex-col gap-1">
    <span class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Settings</span>
    <h1 class="text-3xl font-semibold tracking-tight text-foreground">Account</h1>
  </header>

  <!-- ── Profile card ───────────────────────────────────────────── -->
  <Card.Root>
    <Card.Content class="flex items-center gap-4 py-5">
      <Avatar.Root class="size-12">
        <Avatar.Fallback class="bg-primary text-primary-foreground text-base font-semibold">
          {initial}
        </Avatar.Fallback>
      </Avatar.Root>
      <div class="flex flex-col gap-1.5 min-w-0">
        <p class="font-medium text-foreground truncate">{user?.name ?? 'Unknown'}</p>
        <div class="flex items-center gap-2 flex-wrap">
          <PlanBadge tier={planLabel} />
          {#if showStatus}
            <Badge variant="destructive" class="uppercase tracking-wide text-[10px]">
              {statusLabel.replace('_', ' ')}
            </Badge>
          {/if}
        </div>
      </div>
    </Card.Content>
  </Card.Root>

  <!-- ── Nav links ──────────────────────────────────────────────── -->
  <Item.Group aria-label="Account navigation">
    <Item.Root variant="outline">
      {#snippet child({ props })}
        <a href="/account/billing" {...props}>
          <Item.Media variant="icon" class="bg-primary/10 text-primary size-9 rounded-md">
            <CreditCard class="size-4" />
          </Item.Media>
          <Item.Content>
            <Item.Title>Billing &amp; Plans</Item.Title>
            <Item.Description>Manage your subscription, upgrade, or view invoices.</Item.Description>
          </Item.Content>
          <Item.Actions>
            <ChevronRight class="size-4 text-muted-foreground" />
          </Item.Actions>
        </a>
      {/snippet}
    </Item.Root>

    <Item.Root variant="outline">
      {#snippet child({ props })}
        <a href="/account/usage" {...props}>
          <Item.Media variant="icon" class="bg-primary/10 text-primary size-9 rounded-md">
            <BarChart3 class="size-4" />
          </Item.Media>
          <Item.Content>
            <Item.Title>Usage</Item.Title>
            <Item.Description>View agent turns, token consumption, and storage.</Item.Description>
          </Item.Content>
          <Item.Actions>
            <ChevronRight class="size-4 text-muted-foreground" />
          </Item.Actions>
        </a>
      {/snippet}
    </Item.Root>

    <Item.Root variant="outline" class="[a]:hover:bg-destructive/10 [a]:hover:text-destructive border-destructive/30">
      {#snippet child({ props })}
        <a href={logoutHref} {...props} data-sveltekit-reload>
          <Item.Media variant="icon" class="bg-destructive/10 text-destructive size-9 rounded-md">
            <LogOut class="size-4" />
          </Item.Media>
          <Item.Content>
            <Item.Title class="text-destructive">Sign out</Item.Title>
            <Item.Description>End your session.</Item.Description>
          </Item.Content>
          <Item.Actions>
            <ChevronRight class="size-4 text-destructive/60" />
          </Item.Actions>
        </a>
      {/snippet}
    </Item.Root>
  </Item.Group>

</div>
