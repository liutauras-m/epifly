import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'AgentChatComposer',
  cases: [
    {
      label: 'Idle',
      props: { value: '', attachments: [], inFlight: false,
               onsubmit: () => {}, onUpload: () => {} },
    },
    {
      label: 'Pre-filled',
      props: { value: 'Summarise the latest usage report',
               attachments: [], inFlight: false,
               onsubmit: () => {}, onUpload: () => {} },
    },
    {
      label: 'In-flight (disabled)',
      props: { value: '', attachments: [], inFlight: true,
               onsubmit: () => {}, onUpload: () => {} },
    },
  ],
};
export default fixtures;
