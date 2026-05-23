import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'Composer',
  note: 'Message input primitive. Promoted from AgentChatComposer (Phase 3.5). States: rest, loading, disabled. Supports attachments.',
  cases: [
    { label: 'rest (empty)',       props: { placeholder: 'Ask anything…' } },
    { label: 'loading',            props: { placeholder: 'Ask anything…', loading: true } },
    { label: 'disabled',           props: { placeholder: 'Chat disabled', disabled: true } },
    { label: 'with attachments',   props: {
        placeholder: 'Ask anything…',
        attachments: [
          { id: '1', name: 'design.png',   mimeType: 'image/png'        },
          { id: '2', name: 'spec.pdf',     mimeType: 'application/pdf'  },
        ],
      },
    },
  ],
};
export default fixtures;
