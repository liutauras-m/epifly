import type { ComponentFixtureSet } from '../gallery.types.js';
import type { ChatMessage } from './MessageList.svelte';

const conversation: ChatMessage[] = [
  { role: 'user', text: 'What is the capital of France?' },
  { role: 'ai', text: 'The capital of France is **Paris**. It has been the capital since the 10th century and is home to the Eiffel Tower and the Louvre.' },
  { role: 'user', text: 'And what about Germany?' },
  { role: 'ai', text: 'The capital of Germany is **Berlin**. After reunification in 1990, Berlin was restored as the seat of government.' },
];

const withThinking: ChatMessage[] = [
  { role: 'user', text: 'Explain quantum entanglement simply.' },
  { role: 'thinking', text: '' },
];

const streaming: ChatMessage[] = [
  { role: 'user', text: 'Write a haiku about coding.' },
  {
    role: 'ai',
    text: 'Fingers on',
    streaming: true,
    words: [
      { t: 'Fingers', id: 1, delay: 0 },
      { t: ' on', id: 2, delay: 80 },
    ],
  },
];

const fixtures: ComponentFixtureSet = {
  label: 'MessageList',
  note: 'Scrollable chat message container with auto-scroll and "jump to latest" pill. Renders ChatMessage[] using MessageBubble and ThinkingIndicator.',
  cases: [
    { label: 'Conversation', props: { messages: conversation } },
    { label: 'Thinking', props: { messages: withThinking } },
    { label: 'Streaming', props: { messages: streaming } },
    { label: 'Empty', props: { messages: [] } },
  ],
};

export default fixtures;
