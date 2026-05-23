import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'Field',
  note: 'Form input primitive. Bundles label, input/textarea, hint and error. Pass id= (required for a11y).',
  cases: [
    { label: 'text (default)',      props: { id: 'f1',  label: 'Username',    value: '',              type: 'text',     placeholder: 'e.g. liutauras' } },
    { label: 'email',               props: { id: 'f2',  label: 'Email',       value: '',              type: 'email',    placeholder: 'you@example.com' } },
    { label: 'password',            props: { id: 'f3',  label: 'Password',    value: '',              type: 'password', placeholder: '••••••••' } },
    { label: 'with hint',           props: { id: 'f4',  label: 'Display name', value: '',             hint: 'Shown publicly on your profile.' } },
    { label: 'required',            props: { id: 'f5',  label: 'Work email',  value: '',              type: 'email',    required: true } },
    { label: 'error state',         props: { id: 'f6',  label: 'Email',       value: 'not-an-email',  type: 'email',    error: 'Enter a valid email address.' } },
    { label: 'disabled',            props: { id: 'f7',  label: 'Plan',        value: 'Hobby',         disabled: true } },
    { label: 'multiline (textarea)', props: { id: 'f8', label: 'Notes',       value: '',              multiline: true,  placeholder: 'Add a note…', rows: 4 } },
    { label: 'multiline error',     props: { id: 'f9',  label: 'Bio',         value: 'x'.repeat(180), multiline: true,  error: 'Maximum 160 characters.' } },
  ],
};
export default fixtures;
