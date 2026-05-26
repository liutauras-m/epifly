<script lang="ts">
  import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
  import LogOutIcon from "@lucide/svelte/icons/log-out";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import * as Button from "../ui/button/index.js";
  import * as DropdownMenu from "../ui/dropdown-menu/index.js";
  import * as Separator from "../ui/separator/index.js";
  import UserAvatar from "./user-avatar.svelte";

  type Props = {
    name?: string;
    email?: string;
    avatarUrl?: string;
    onLogout?: () => void | Promise<void>;
    onSettings?: () => void;
  };

  let { name, email, avatarUrl, onLogout, onSettings }: Props = $props();
</script>

<DropdownMenu.DropdownMenu>
  <DropdownMenu.DropdownMenuTrigger>
    {#snippet child({ props })}
      <Button.Button
        {...props}
        type="button"
        variant="ghost"
        class="h-auto w-full justify-start gap-3 px-2 py-2"
        aria-label="Account menu"
      >
        <UserAvatar {name} {email} {avatarUrl} />
        <div class="flex min-w-0 flex-1 flex-col text-left">
          {#if name}
            <span class="truncate text-sm font-medium leading-none">{name}</span>
          {/if}
          {#if email}
            <span class="truncate text-xs text-muted-foreground leading-none mt-0.5">{email}</span>
          {/if}
        </div>
        <ChevronsUpDownIcon class="ml-auto size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} aria-hidden="true" />
      </Button.Button>
    {/snippet}
  </DropdownMenu.DropdownMenuTrigger>

  <DropdownMenu.DropdownMenuContent class="w-56" side="top" align="start">
    <div class="px-2 py-1.5">
      {#if name}
        <p class="text-sm font-medium">{name}</p>
      {/if}
      {#if email}
        <p class="text-xs text-muted-foreground">{email}</p>
      {/if}
    </div>
    <Separator.Separator />
    {#if onSettings}
      <DropdownMenu.DropdownMenuItem onclick={onSettings}>
        <SettingsIcon class="mr-2 size-4" strokeWidth={1.75} aria-hidden="true" />
        Settings
      </DropdownMenu.DropdownMenuItem>
    {/if}
    {#if onLogout}
      <DropdownMenu.DropdownMenuItem onclick={onLogout} class="text-destructive focus:text-destructive">
        <LogOutIcon class="mr-2 size-4" strokeWidth={1.75} aria-hidden="true" />
        Log out
      </DropdownMenu.DropdownMenuItem>
    {/if}
  </DropdownMenu.DropdownMenuContent>
</DropdownMenu.DropdownMenu>
