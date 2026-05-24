import type { ComponentFixtureSet } from '../gallery.types.js';
import { Search, Settings, ChevronRight, X, Plus, Send, Paperclip, Zap, LayoutGrid, User } from '@lucide/svelte';

const fixtures: ComponentFixtureSet = {
  label: 'Icon',
  note: 'stroke-width 1.5 enforced. Size sm=16 md=20 lg=24. Pass label= for standalone icons.',
  cases: [
    { label: 'sm (16)',   props: { icon: Search,      size: 'sm' } },
    { label: 'md (20)',   props: { icon: Settings,    size: 'md' } },
    { label: 'lg (24)',   props: { icon: ChevronRight, size: 'lg' } },
    { label: 'With label', props: { icon: Search, size: 'md', label: 'Search' } },
    { label: 'X / close',  props: { icon: X,    size: 'sm' } },
    { label: 'Plus',        props: { icon: Plus, size: 'md' } },
    { label: 'Send',        props: { icon: Send, size: 'md' } },
    { label: 'Paperclip',  props: { icon: Paperclip, size: 'md' } },
    { label: 'Zap',        props: { icon: Zap,        size: 'md' } },
    { label: 'Grid',       props: { icon: LayoutGrid, size: 'md' } },
    { label: 'User',       props: { icon: User,       size: 'md' } },
  ],
};
export default fixtures;
