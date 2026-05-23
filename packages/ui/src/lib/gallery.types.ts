/**
 * Gallery fixture type definitions (Phase 2.6).
 *
 * Every primitive in packages/ui/src/lib/components/ ships a sibling
 * `ComponentName.fixtures.ts` that satisfies this shape.  The `/_/ui`
 * gallery page in apps/web consumes them to render each component with
 * representative prop sets for visual review.
 */

export interface FixtureCase {
  /** Short label shown above the rendered instance. */
  label: string;
  /** Props to spread onto the component. Use `{}` for no-prop components. */
  props: Record<string, unknown>;
}

export interface ComponentFixtureSet {
  /** Display label for the component section heading. */
  label: string;
  /** Optional note rendered beneath the heading (e.g., context requirements). */
  note?: string;
  /** One or more prop sets to render side-by-side. */
  cases: FixtureCase[];
}
