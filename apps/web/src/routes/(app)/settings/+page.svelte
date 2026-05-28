<script lang="ts">
  import { AppSafeArea } from "@epifly/ui";
</script>

<svelte:head>
  <title>Settings · Epifly</title>
</svelte:head>

<AppSafeArea class="flex min-h-0 flex-1 flex-col overflow-y-auto">
  <div class="mx-auto w-full max-w-2xl px-6 pb-8 pt-[calc(var(--sidebar-toggle-offset)+2.75rem)]">
    <h1 class="mb-8 text-2xl font-semibold tracking-tight">Settings</h1>

    <div class="space-y-8">
      <!-- Account -->
      <section aria-labelledby="settings-account">
        <h2 id="settings-account" class="mb-3 text-xs font-semibold uppercase tracking-widest text-muted-foreground">
          Account
        </h2>
        <div class="divide-y divide-border rounded-xl border bg-card">
          <div class="flex items-center justify-between px-4 py-3.5">
            <span class="text-sm font-medium">Name</span>
            <span class="text-sm text-muted-foreground">—</span>
          </div>
          <div class="flex items-center justify-between px-4 py-3.5">
            <span class="text-sm font-medium">Email</span>
            <span class="text-sm text-muted-foreground">—</span>
          </div>
          <div class="flex items-center justify-between px-4 py-3.5">
            <span class="text-sm font-medium">Plan</span>
            <span class="text-sm text-muted-foreground">—</span>
          </div>
        </div>
      </section>

      <!-- API / Auth -->
      <section aria-labelledby="settings-api">
        <h2 id="settings-api" class="mb-3 text-xs font-semibold uppercase tracking-widest text-muted-foreground">
          API
        </h2>
        <div class="divide-y divide-border rounded-xl border bg-card">
          <div class="flex items-center justify-between px-4 py-3.5">
            <div>
              <p class="text-sm font-medium">Authentication</p>
              <p class="mt-0.5 text-xs text-muted-foreground">
                Token provider not yet configured.
              </p>
            </div>
            <span class="rounded-full bg-muted px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
              Pending
            </span>
          </div>
        </div>
      </section>

      <!-- Appearance -->
      <section aria-labelledby="settings-appearance">
        <h2 id="settings-appearance" class="mb-3 text-xs font-semibold uppercase tracking-widest text-muted-foreground">
          Appearance
        </h2>
        <div class="divide-y divide-border rounded-xl border bg-card">
          <div class="flex items-center justify-between px-4 py-3.5">
            <span class="text-sm font-medium">Theme</span>
            <span class="text-sm text-muted-foreground">System</span>
          </div>
        </div>
      </section>
    </div>
  </div>
</AppSafeArea>
