<svelte:options runes={true} />
<script lang="ts">
  /**
   * Field — form input primitive (Phase 2.7).
   *
   * Canonical form field that bundles `<label>`, `<input>` / `<textarea>`,
   * hint text and error state in one unit.  All login, settings and billing
   * forms in `apps/*` must use this component — never raw `<input>` with
   * local CSS.
   *
   * Usage:
   *   <Field id="email" label="Email" type="email" required />
   *   <Field id="bio"   label="Bio"   multiline    hint="Max 160 chars" />
   *   <Field id="pw"    label="Password" type="password" error="Too short" />
   *
   * A11y: `id` is required.  The component wires `for` → `aria-describedby`
   * automatically when `hint` or `error` are present.
   */

  export type FieldType =
    | 'text' | 'email' | 'password' | 'number'
    | 'tel'  | 'url'   | 'search'   | 'date';

  let {
    id,
    label,
    type          = 'text' as FieldType,
    value         = $bindable(''),
    placeholder   = '',
    hint,
    error,
    required      = false,
    disabled      = false,
    readonly      = false,
    multiline     = false,
    rows          = 3,
    autocomplete,
    class: cls    = '',
    oninput,
    onchange,
    onblur,
    onfocus,
    ...rest
  }: {
    id:            string;
    label:         string;
    type?:         FieldType;
    value?:        string;
    placeholder?:  string;
    hint?:         string;
    error?:        string;
    required?:     boolean;
    disabled?:     boolean;
    readonly?:     boolean;
    /** Render a <textarea> instead of <input>. */
    multiline?:    boolean;
    rows?:         number;
    autocomplete?: string;
    class?:        string;
    oninput?:      (e: Event) => void;
    onchange?:     (e: Event) => void;
    onblur?:       (e: FocusEvent) => void;
    onfocus?:      (e: FocusEvent) => void;
    [key: string]: unknown;
  } = $props();

  const descId   = $derived(hint || error ? `${id}-desc` : undefined);
  const hasError = $derived(Boolean(error));
</script>

<div
  class="field{hasError ? ' field-error' : ''}{disabled ? ' field-disabled' : ''}{cls ? ` ${cls}` : ''}"
>
  <!-- Label -->
  <label class="field-label" for={id}>
    {label}
    {#if required}<span class="field-required" aria-hidden="true">*</span>{/if}
  </label>

  <!-- Control -->
  {#if multiline}
    <textarea
      {id}
      class="field-control"
      {value}
      {placeholder}
      {required}
      {disabled}
      {readonly}
      {rows}
      aria-invalid={hasError || undefined}
      aria-describedby={descId}
      {oninput}
      {onchange}
      {onblur}
      {onfocus}
      {...rest}
    ></textarea>
  {:else}
    <input
      {id}
      class="field-control"
      {type}
      {value}
      {placeholder}
      {required}
      {disabled}
      {readonly}
      {autocomplete}
      aria-invalid={hasError || undefined}
      aria-describedby={descId}
      {oninput}
      {onchange}
      {onblur}
      {onfocus}
      {...rest}
    />
  {/if}

  <!-- Hint / error -->
  {#if error || hint}
    <p id={descId} class="field-desc" role={hasError ? 'alert' : undefined}>
      {error ?? hint}
    </p>
  {/if}
</div>

<style>
  /* ── Container ───────────────────────────────────────────────────────────── */
  .field {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-1);
    width:          100%;
  }

  /* ── Label ───────────────────────────────────────────────────────────────── */
  .field-label {
    font-size:      var(--font-size-meta);   /* 13px */
    font-weight:    500;
    color:          var(--color-fg-muted);
    line-height:    1.4;
    user-select:    none;
  }

  .field-required {
    color:   var(--color-accent);
    margin-left: 2px;
  }

  /* ── Control (shared input + textarea) ──────────────────────────────────── */
  .field-control {
    width:          100%;
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-body);   /* 15px */
    color:          var(--color-fg);
    background:     var(--color-bg-raised);
    border:         1px solid var(--color-border);
    border-radius:  var(--radius-sm);
    padding:        var(--space-2) var(--space-3);
    outline:        none;
    line-height:    1.5;

    transition:
      border-color  var(--duration-fast) var(--ease-standard),
      box-shadow    var(--duration-fast) var(--ease-standard);

    /* prevent iOS zoom (font-size must be ≥ 16px effective) */
    font-size: max(16px, var(--font-size-body));
  }

  .field-control:hover:not(:disabled) {
    border-color: var(--color-border-strong);
  }

  .field-control:focus {
    border-color: var(--color-accent);
    box-shadow:   0 0 0 var(--focus-ring-offset) var(--color-bg),
                  0 0 0 calc(var(--focus-ring-offset) + 2px) var(--color-accent);
  }

  .field-control:disabled,
  .field-disabled .field-control {
    opacity:  0.5;
    cursor:   not-allowed;
    background: var(--color-bg);
  }

  textarea.field-control {
    resize:     vertical;
    min-height: calc(var(--space-4) * 3 + var(--font-size-body) * 1.5 * 3);
  }

  /* ── Error state ─────────────────────────────────────────────────────────── */
  .field-error .field-control {
    border-color: var(--color-danger);
  }
  .field-error .field-control:focus {
    box-shadow: 0 0 0 var(--focus-ring-offset) var(--color-bg),
                0 0 0 calc(var(--focus-ring-offset) + 2px) var(--color-danger);
  }

  /* ── Hint / error text ───────────────────────────────────────────────────── */
  .field-desc {
    margin:      0;
    font-size:   var(--font-size-meta);   /* 13px */
    line-height: 1.4;
    color:       var(--color-fg-subtle);
  }

  .field-error .field-desc {
    color: var(--color-danger);
  }
</style>
