import type { ComponentFixtureSet } from '../gallery.types.js';
import { Zap, FlaskConical, Globe } from 'lucide-svelte';

const fixtures: ComponentFixtureSet = {
  label: 'Chip',
  note: 'Filter/tag primitive. Variants: tonal (default), outlined. Sizes: sm, md. Supports selected, removable, leading icon.',
  cases: [
    // ── Tonal (default) ───────────────────────────────────────────────────
    { label: 'tonal md',           props: { label: 'All',            variant: 'tonal',    size: 'md' } },
    { label: 'tonal selected',     props: { label: 'Active',         variant: 'tonal',    size: 'md', selected: true } },
    { label: 'tonal sm',           props: { label: 'Beta',           variant: 'tonal',    size: 'sm' } },
    { label: 'tonal sm selected',  props: { label: 'Featured',       variant: 'tonal',    size: 'sm', selected: true } },
    // ── Outlined ─────────────────────────────────────────────────────────
    { label: 'outlined',           props: { label: 'Image gen',      variant: 'outlined', size: 'md' } },
    { label: 'outlined selected',  props: { label: 'Code',           variant: 'outlined', size: 'md', selected: true } },
    { label: 'outlined sm',        props: { label: 'Pinned',         variant: 'outlined', size: 'sm' } },
    // ── With icons ────────────────────────────────────────────────────────
    { label: 'icon tonal',         props: { label: 'AI',             variant: 'tonal',    size: 'md', icon: Zap } },
    { label: 'icon selected',      props: { label: 'Science',        variant: 'tonal',    size: 'md', icon: FlaskConical, selected: true } },
    { label: 'icon outlined sm',   props: { label: 'Web',            variant: 'outlined', size: 'sm', icon: Globe } },
    // ── Removable ─────────────────────────────────────────────────────────
    { label: 'removable tonal',    props: { label: 'python',         variant: 'tonal',    size: 'md', onremove: () => {} } },
    { label: 'removable outlined', props: { label: 'stable-diffusion', variant: 'outlined', size: 'md', onremove: () => {} } },
  ],
};
export default fixtures;
