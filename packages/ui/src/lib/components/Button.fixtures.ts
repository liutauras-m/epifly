import type { ComponentFixtureSet } from '../gallery.types.js';
import { ArrowRight, Trash2, Check, Loader2, Send, Plus } from 'lucide-svelte';

const fixtures: ComponentFixtureSet = {
  label: 'Button',
  note: 'Variants: primary, secondary, ghost, danger, outline. Sizes: sm, md, lg. Supports loading, disabled, icon slots.',
  cases: [
    // ── Variants (md) ────────────────────────────────────────────────────────
    { label: 'primary',            props: { variant: 'primary',   size: 'md', text: 'Save changes' } },
    { label: 'secondary',          props: { variant: 'secondary', size: 'md', text: 'Cancel' } },
    { label: 'ghost',              props: { variant: 'ghost',     size: 'md', text: 'Learn more' } },
    { label: 'danger',             props: { variant: 'danger',    size: 'md', text: 'Delete account' } },
    { label: 'outline',            props: { variant: 'outline',   size: 'md', text: 'View plan' } },
    // ── Sizes ─────────────────────────────────────────────────────────────────
    { label: 'primary sm',         props: { variant: 'primary',   size: 'sm', text: 'Small' } },
    { label: 'primary lg',         props: { variant: 'primary',   size: 'lg', text: 'Large' } },
    // ── States ────────────────────────────────────────────────────────────────
    { label: 'disabled',           props: { variant: 'primary',   size: 'md', text: 'Disabled', disabled: true } },
    { label: 'loading',            props: { variant: 'primary',   size: 'md', text: 'Saving…',  loading: true } },
    // ── With icons ────────────────────────────────────────────────────────────
    { label: 'icon leading',       props: { variant: 'primary',   size: 'md', text: 'Send',       iconLeading: Send } },
    { label: 'icon trailing',      props: { variant: 'secondary', size: 'md', text: 'Continue',   iconTrailing: ArrowRight } },
    { label: 'ghost icon leading', props: { variant: 'ghost',     size: 'sm', text: 'Add item',   iconLeading: Plus } },
    { label: 'danger icon',        props: { variant: 'danger',    size: 'md', text: 'Delete',      iconLeading: Trash2 } },
    { label: 'success outline',    props: { variant: 'outline',   size: 'md', text: 'Confirm',     iconLeading: Check } },
    // ── Full width ────────────────────────────────────────────────────────────
    { label: 'full-width (submit)', props: { variant: 'primary',  size: 'lg', text: 'Continue to billing', fullWidth: true } },
  ],
};
export default fixtures;
