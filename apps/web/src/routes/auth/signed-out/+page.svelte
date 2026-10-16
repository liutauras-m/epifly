<svelte:options runes={true} />
<script lang="ts">
  import { Button } from '$lib/components/ui/button/index.js';
  import * as Card from '$lib/components/ui/card/index.js';
</script>

<svelte:head><title>Signed out · ConusAI</title></svelte:head>

<main class="min-h-dvh flex items-center justify-center px-4">
  <Card.Root class="w-full max-w-sm">
    <Card.Header>
      <Card.Title>You are signed out</Card.Title>
      <Card.Description>Your session has ended on this device.</Card.Description>
    </Card.Header>
    <Card.Footer>
      <Button href="/auth/login" class="w-full" data-sveltekit-reload>Sign in again</Button>
    </Card.Footer>
  </Card.Root>
</main>
