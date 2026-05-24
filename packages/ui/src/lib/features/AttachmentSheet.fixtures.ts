import type { ComponentProps } from 'svelte';
import type AttachmentSheet from './AttachmentSheet.svelte';
import type { Attachment } from '../components/Composer.svelte';

type Props = ComponentProps<typeof AttachmentSheet>;

const base: Props = {
  open: true,
  onclose: () => {},
  onAdd: (_atts: Attachment[]) => {},
  onUpload: async (_files: File[]) => [],
};

const fixtures: { label: string; props: Props }[] = [
  { label: 'Open', props: { ...base } },
  { label: 'Closed', props: { ...base, open: false } },
];

export default fixtures;
