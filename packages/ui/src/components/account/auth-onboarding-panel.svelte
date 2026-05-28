<script lang="ts">
  import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
  import CheckIcon from "@lucide/svelte/icons/check";
  import FileTextIcon from "@lucide/svelte/icons/file-text";
  import KeyRoundIcon from "@lucide/svelte/icons/key-round";
  import MailIcon from "@lucide/svelte/icons/mail";
  import MessageCircleIcon from "@lucide/svelte/icons/message-circle";
  import ShieldCheckIcon from "@lucide/svelte/icons/shield-check";
  import SparklesIcon from "@lucide/svelte/icons/sparkles";
  import { cn } from "../../utils/cn.js";
  import AiToolProgress from "../app/ai-tool-progress.svelte";

  type Props = {
    class?: string;
    prompt?: string;
    email?: string;
    password?: string;
    error?: string | null;
    isSubmitting?: boolean;
    onEmailChange?: (value: string) => void;
    onPasswordChange?: (value: string) => void;
    onSubmit?: () => void | Promise<void>;
  };

  let {
    class: className,
    prompt = "hello",
    email = "",
    password = "",
    error = null,
    isSubmitting = false,
    onEmailChange,
    onPasswordChange,
    onSubmit
  }: Props = $props();

  let mode = $state<"signup" | "signin">("signup");

  const isSignup = $derived(mode === "signup");
  const eyebrow = $derived(isSignup ? "Continue in Epifly" : "Welcome back");
  const title = $derived(isSignup ? "Create your account" : "Sign in to Epifly");
  const description = $derived(
    isSignup
      ? "Save this conversation, keep your context, and pick up exactly where you left off."
      : "Return to your saved conversations, files, and workspace context."
  );
  const primaryLabel = $derived(isSignup ? "Sign up for free" : "Continue");
  const emailLabel = $derived(isSignup ? "Continue with email" : "Sign in with email");
  const passkeyLabel = $derived(isSignup ? "Use passkey" : "Use saved passkey");
  const switchPrompt = $derived(isSignup ? "Already have an account?" : "New to Epifly?");
  const switchLabel = $derived(isSignup ? "Sign in" : "Create one");
  const canSubmit = $derived(email.trim().length > 3 && password.trim().length > 0 && !isSubmitting);

  const benefits = [
    { icon: SparklesIcon, label: "Keep your conversation moving" },
    { icon: MessageCircleIcon, label: "Chat history across devices" },
    { icon: FileTextIcon, label: "Files and workspace context saved" },
    { icon: ShieldCheckIcon, label: "Private by default" }
  ];

  function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    void onSubmit?.();
  }
</script>

<section class={cn("auth-onboarding", className)} aria-labelledby="auth-onboarding-title">
  <div class="auth-onboarding__topbar" aria-label="Epifly account">
    <AiToolProgress state="idle" size="sm" showLabel={false} />
    <span class="auth-onboarding__brand">Epifly</span>
  </div>

  <div class="auth-onboarding__stage">
    <div class="auth-onboarding__conversation" aria-label="Conversation preview">
      <div class="auth-onboarding__user-bubble">{prompt}</div>
      <div class="auth-onboarding__continue-card">
        <div>
          <p class="auth-onboarding__eyebrow">{eyebrow}</p>
          <h1 id="auth-onboarding-title">{title}</h1>
          <p>{description}</p>
        </div>

        <form class="auth-onboarding__form" onsubmit={handleSubmit}>
          <div class="auth-onboarding__fields">
            <label>
              <span>Email</span>
              <input
                type="email"
                autocomplete={isSignup ? "email" : "username"}
                inputmode="email"
                value={email}
                disabled={isSubmitting}
                oninput={(e) => onEmailChange?.((e.currentTarget as HTMLInputElement).value)}
                placeholder="you@example.com"
              />
            </label>
            <label>
              <span>Password</span>
              <input
                type="password"
                autocomplete={isSignup ? "new-password" : "current-password"}
                value={password}
                disabled={isSubmitting}
                oninput={(e) => onPasswordChange?.((e.currentTarget as HTMLInputElement).value)}
                placeholder="Enter password"
              />
            </label>
          </div>

          {#if error}
            <p class="auth-onboarding__error" role="alert">{error}</p>
          {/if}

          <div class="auth-onboarding__actions" aria-label="Account actions">
            <button class="auth-onboarding__primary" type="submit" disabled={!canSubmit}>
              {isSubmitting ? "Connecting" : primaryLabel}
              <ArrowRightIcon size={16} strokeWidth={1.8} aria-hidden="true" />
            </button>
            <button class="auth-onboarding__secondary" type="submit" disabled={!canSubmit}>
              <MailIcon size={16} strokeWidth={1.75} aria-hidden="true" />
              {emailLabel}
            </button>
            <button
              class="auth-onboarding__secondary"
              type="button"
              disabled
              title="Passkey sign-in is coming soon"
              aria-label={`${passkeyLabel} is coming soon`}
            >
              <KeyRoundIcon size={16} strokeWidth={1.75} aria-hidden="true" />
              {passkeyLabel} · Soon
            </button>
          </div>
        </form>

        <p class="auth-onboarding__legal">
          By continuing, you agree to Epifly's
          <a href="/terms">Terms</a>
          and
          <a href="/privacy">Privacy Policy</a>.
        </p>
      </div>
    </div>

    <aside class="auth-onboarding__value" aria-label="Account benefits">
      <div class="auth-onboarding__value-header">
        <AiToolProgress state="thinking" variant="pill" label="Ready" showLabel={true} />
        <p>What your account unlocks</p>
      </div>

      <ul>
        {#each benefits as benefit}
          {@const BenefitIcon = benefit.icon}
          <li>
            <span aria-hidden="true">
              <BenefitIcon size={17} strokeWidth={1.7} />
            </span>
            {benefit.label}
          </li>
        {/each}
      </ul>

      <div class="auth-onboarding__trust">
        <CheckIcon size={15} strokeWidth={1.8} aria-hidden="true" />
        <span>No credit card needed</span>
      </div>
    </aside>
  </div>

  <p class="auth-onboarding__signin">
    {switchPrompt}
    <button type="button" onclick={() => (mode = isSignup ? "signin" : "signup")}>{switchLabel}</button>
  </p>
</section>

<style>
  .auth-onboarding {
    min-height: 100svh;
    padding: max(1rem, var(--safe-top)) max(1rem, var(--safe-right)) max(1rem, var(--safe-bottom)) max(1rem, var(--safe-left));
    color: var(--foreground);
    background:
      radial-gradient(circle at 12% 8%, color-mix(in oklch, var(--epifly-tool-cyan) 8%, transparent), transparent 30%),
      radial-gradient(circle at 86% 16%, color-mix(in oklch, var(--epifly-logo-orange) 11%, transparent), transparent 30%),
      linear-gradient(180deg, color-mix(in oklch, var(--background) 98%, var(--epifly-logo-orange) 2%), var(--background));
  }

  .auth-onboarding__topbar {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    min-height: 2.25rem;
  }

  .auth-onboarding__brand {
    font-size: 0.875rem;
    font-weight: 700;
    letter-spacing: -0.04em;
  }

  .auth-onboarding__stage {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1rem;
    width: min(56rem, calc(100vw - 2rem));
    margin: clamp(2.25rem, 9vh, 6.25rem) auto 0;
  }

  .auth-onboarding__conversation {
    display: grid;
    justify-items: center;
    gap: 0.65rem;
    min-width: 0;
  }

  .auth-onboarding__user-bubble {
    max-width: min(16rem, 80vw);
    border: 1px solid color-mix(in oklch, var(--border) 78%, transparent);
    border-radius: 999px;
    background: color-mix(in oklch, var(--background) 94%, white);
    box-shadow: 0 0.75rem 2rem color-mix(in oklch, var(--foreground) 5%, transparent);
    padding: 0.6rem 0.9rem;
    font-size: 0.875rem;
    line-height: 1;
  }

  .auth-onboarding__continue-card,
  .auth-onboarding__value {
    border: 1px solid color-mix(in oklch, var(--border) 82%, transparent);
    border-radius: 1.25rem;
    background: color-mix(in oklch, var(--background) 88%, white);
    box-shadow:
      0 0.0625rem 0.125rem color-mix(in oklch, var(--foreground) 5%, transparent),
      0 1.25rem 3.5rem color-mix(in oklch, var(--foreground) 7%, transparent);
    backdrop-filter: blur(20px) saturate(1.04);
  }

  .auth-onboarding__continue-card {
    display: grid;
    gap: 1.35rem;
    width: min(31rem, 100%);
    padding: clamp(1.1rem, 3vw, 1.5rem);
  }

  .auth-onboarding__eyebrow {
    margin: 0 0 0.55rem;
    font-size: 0.7rem;
    font-weight: 650;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--epifly-logo-orange-hot);
  }

  .auth-onboarding h1 {
    margin: 0;
    font-size: clamp(1.65rem, 4vw, 2.35rem);
    font-weight: 760;
    letter-spacing: -0.04em;
    line-height: 0.98;
  }

  .auth-onboarding h1 + p {
    max-width: 28rem;
    margin: 0.65rem 0 0;
    color: var(--muted-foreground);
    font-size: 0.925rem;
    line-height: 1.55;
  }

  .auth-onboarding__actions {
    display: grid;
    gap: 0.6rem;
  }

  .auth-onboarding__form,
  .auth-onboarding__fields {
    display: grid;
    gap: 0.75rem;
  }

  .auth-onboarding__fields label {
    display: grid;
    gap: 0.4rem;
  }

  .auth-onboarding__fields span {
    color: var(--muted-foreground);
    font-size: 0.72rem;
    font-weight: 650;
  }

  .auth-onboarding__fields input {
    min-height: 2.75rem;
    border: 1px solid color-mix(in oklch, var(--border) 86%, var(--foreground) 6%);
    border-radius: 999px;
    background: color-mix(in oklch, var(--background) 96%, white);
    color: var(--foreground);
    font-size: 0.875rem;
    outline: none;
    padding: 0 1rem;
    transition:
      border-color var(--motion-fast) var(--ease-standard),
      box-shadow var(--motion-fast) var(--ease-standard);
  }

  .auth-onboarding__fields input:focus {
    border-color: color-mix(in oklch, var(--epifly-tool-cyan) 60%, var(--border));
    box-shadow: 0 0 0 0.2rem color-mix(in oklch, var(--epifly-tool-cyan) 12%, transparent);
  }

  .auth-onboarding__fields input:disabled,
  .auth-onboarding__primary:disabled,
  .auth-onboarding__secondary:disabled {
    cursor: not-allowed;
    opacity: 0.58;
  }

  .auth-onboarding__error {
    margin: 0;
    border-radius: 0.875rem;
    background: color-mix(in oklch, var(--destructive) 8%, var(--background));
    color: var(--destructive);
    font-size: 0.78rem;
    line-height: 1.4;
    padding: 0.65rem 0.75rem;
  }

  .auth-onboarding__primary,
  .auth-onboarding__secondary {
    display: inline-flex;
    min-height: 2.75rem;
    width: 100%;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    border-radius: 999px;
    font-size: 0.875rem;
    font-weight: 650;
    letter-spacing: 0;
    transition:
      transform var(--motion-fast) var(--ease-standard),
      box-shadow var(--motion-fast) var(--ease-standard),
      border-color var(--motion-fast) var(--ease-standard),
      background var(--motion-fast) var(--ease-standard);
  }

  .auth-onboarding__primary {
    border: 1px solid var(--foreground);
    background: var(--foreground);
    color: var(--background);
    box-shadow: 0 0.875rem 1.875rem color-mix(in oklch, var(--foreground) 16%, transparent);
  }

  .auth-onboarding__secondary {
    border: 1px solid color-mix(in oklch, var(--border) 88%, var(--foreground) 8%);
    background: color-mix(in oklch, var(--background) 94%, white);
    color: var(--foreground);
  }

  .auth-onboarding__primary:hover,
  .auth-onboarding__secondary:hover {
    transform: translateY(-0.0625rem);
  }

  .auth-onboarding__primary:active,
  .auth-onboarding__secondary:active {
    transform: translateY(0);
  }

  .auth-onboarding__primary:focus-visible,
  .auth-onboarding__secondary:focus-visible,
  .auth-onboarding__fields input:focus-visible,
  .auth-onboarding a:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--epifly-tool-cyan) 68%, transparent);
    outline-offset: 3px;
  }

  .auth-onboarding__legal,
  .auth-onboarding__signin {
    margin: 0;
    color: var(--muted-foreground);
    font-size: 0.72rem;
    line-height: 1.55;
    text-align: center;
  }

  .auth-onboarding a {
    color: var(--foreground);
    font-weight: 650;
    text-decoration: none;
  }

  .auth-onboarding__signin button {
    border: 0;
    background: transparent;
    color: var(--foreground);
    font: inherit;
    font-weight: 650;
    padding: 0;
  }

  .auth-onboarding a:hover,
  .auth-onboarding__signin button:hover {
    text-decoration: underline;
    text-underline-offset: 0.18em;
  }

  .auth-onboarding__value {
    display: none;
    min-width: 0;
    padding: 1.15rem;
  }

  .auth-onboarding__value-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    border-bottom: 1px solid color-mix(in oklch, var(--border) 72%, transparent);
    padding-bottom: 1rem;
  }

  .auth-onboarding__value-header p {
    margin: 0;
    color: var(--muted-foreground);
    font-size: 0.75rem;
    font-weight: 600;
  }

  .auth-onboarding__value ul {
    display: grid;
    gap: 0.9rem;
    margin: 1.05rem 0 0;
    padding: 0;
    list-style: none;
  }

  .auth-onboarding__value li {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    font-size: 0.83rem;
    font-weight: 600;
  }

  .auth-onboarding__value li span,
  .auth-onboarding__trust {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .auth-onboarding__value li span {
    width: 1.75rem;
    height: 1.75rem;
    flex: 0 0 auto;
    border-radius: 999px;
    background: color-mix(in oklch, var(--epifly-logo-orange) 9%, var(--background));
    color: var(--epifly-logo-orange-hot);
  }

  .auth-onboarding__trust {
    width: fit-content;
    gap: 0.35rem;
    margin-top: 1.2rem;
    border-radius: 999px;
    background: color-mix(in oklch, var(--epifly-tool-cyan) 8%, var(--background));
    padding: 0.45rem 0.6rem;
    color: var(--muted-foreground);
    font-size: 0.72rem;
    font-weight: 650;
  }

  .auth-onboarding__signin {
    margin-top: 1.25rem;
  }

  :global(.dark) .auth-onboarding {
    background:
      radial-gradient(circle at 12% 8%, color-mix(in oklch, var(--epifly-tool-cyan) 14%, transparent), transparent 30%),
      radial-gradient(circle at 86% 16%, color-mix(in oklch, var(--epifly-logo-orange) 14%, transparent), transparent 30%),
      linear-gradient(180deg, color-mix(in oklch, var(--background) 95%, white 3%), var(--background));
  }

  :global(.dark) .auth-onboarding__continue-card,
  :global(.dark) .auth-onboarding__value,
  :global(.dark) .auth-onboarding__user-bubble,
  :global(.dark) .auth-onboarding__secondary {
    background: color-mix(in oklch, var(--background) 88%, white 4%);
  }

  @media (min-width: 760px) {
    .auth-onboarding__stage {
      grid-template-columns: minmax(0, 1.2fr) minmax(17rem, 0.8fr);
      align-items: center;
    }

    .auth-onboarding__conversation {
      justify-items: end;
    }

    .auth-onboarding__value {
      display: block;
    }
  }
</style>