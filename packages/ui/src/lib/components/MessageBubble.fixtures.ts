import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'MessageBubble',
  note: 'Single chat message bubble. User messages: right-aligned accent fill. Assistant: left-aligned with AI mark and markdown prose.',
  cases: [
    {
      label: 'User message',
      props: { role: 'user', text: 'How do I set up a SvelteKit project?' },
    },
    {
      label: 'Assistant message',
      props: {
        role: 'assistant',
        text: 'Run `npm create svelte@latest my-app`, then `cd my-app && npm install`. You can choose between a skeleton, demo, or library template.',
      },
    },
    {
      label: 'Streaming (no words yet)',
      props: { role: 'assistant', text: '', streaming: true, words: [] },
    },
    {
      label: 'Streaming (partial)',
      props: {
        role: 'assistant',
        text: 'Sure, here',
        streaming: true,
        words: [
          { t: 'Sure', id: 1, delay: 0 },
          { t: ',', id: 2, delay: 60 },
          { t: ' here', id: 3, delay: 120 },
        ],
      },
    },
  ],
};

export default fixtures;
