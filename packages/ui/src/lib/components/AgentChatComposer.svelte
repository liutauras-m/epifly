<svelte:options runes={true} />
<script lang="ts">
  /**
   * @deprecated Use Composer (Phase 3.5) — deleted at Phase 4 close via ui:contracts gate.
   *
   * Migration shim: delegates to the canonical Composer component.
   * Adapter: inFlight → loading, Attachment shape maps to Composer.Attachment.
   */
  import Composer from './Composer.svelte';
  import type { Attachment as ComposerAttachment } from './Composer.svelte';

  /** @deprecated Legacy Attachment type — use Composer.Attachment going forward. */
  export interface Attachment {
    id:       string;
    filename: string;
    size:     number;
  }

  let {
    value       = $bindable(''),
    attachments = $bindable<Attachment[]>([]),
    inFlight    = false,
    onsubmit,
    onUpload,
  }: {
    value?:       string;
    attachments?: Attachment[];
    inFlight?:    boolean;
    onsubmit:     (prompt: string, attachments: Attachment[]) => void;
    onUpload?:    (files: File[]) => Promise<Attachment[]>;
  } = $props();

  // Map Composer.Attachment → legacy Attachment on submit
  let composerAttachments = $state<ComposerAttachment[]>([]);

  function handleSubmit(prompt: string, atts: ComposerAttachment[]) {
    const legacy: Attachment[] = atts.map(a => ({ id: a.id, filename: a.name, size: 0 }));
    onsubmit(prompt, legacy);
  }

  async function handleUpload(files: File[]): Promise<ComposerAttachment[]> {
    if (!onUpload) return [];
    const legacyAtts = await onUpload(files);
    return legacyAtts.map(a => ({ id: a.id, name: a.filename, mimeType: undefined }));
  }
</script>

<Composer
  bind:value
  bind:attachments={composerAttachments}
  loading={inFlight}
  onsubmit={handleSubmit}
  onUpload={onUpload ? handleUpload : undefined}
/>
