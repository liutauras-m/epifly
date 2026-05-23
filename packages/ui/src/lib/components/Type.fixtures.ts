import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'Type',
  note: 'The ONLY place font-variation-settings lives. Body copy uses <p class="t-body"> instead.',
  cases: [
    { label: 'display',  props: { variant: 'display', text: 'Good evening, Liutauras.' } },
    { label: 'h1',       props: { variant: 'h1',      text: 'Section Title'            } },
    { label: 'h2',       props: { variant: 'h2',      text: 'Subsection Header'        } },
    { label: 'h3',       props: { variant: 'h3',      text: 'Card / Panel Title'       } },
    { label: 'label',    props: { variant: 'label',   text: 'Recent Chats'             } },
    { label: 'meta',     props: { variant: 'meta',    text: '2 min ago · 3.2k tokens'  } },
    { label: 'mono',     props: { variant: 'mono',    text: 'cap-001 · ws-acme'        } },
  ],
};
export default fixtures;
