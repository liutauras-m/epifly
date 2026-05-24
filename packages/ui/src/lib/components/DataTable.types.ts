export interface Column {
  key:       string;
  label:     string;
  sortable?: boolean;
  align?:    'left' | 'center' | 'right';
  /** Hidden label for the card layout (default: same as label). */
  cardLabel?: string;
}
