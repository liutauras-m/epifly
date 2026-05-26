/**
 * Shared types used across web, native, and feature packages.
 * Must not reference browser-only or Tauri-only APIs.
 */

export type Nullable<T> = T | null;
export type Optional<T> = T | undefined;

/** Generic paginated list envelope. */
export interface PaginatedList<T> {
  data: T[];
  total?: number;
  hasMore?: boolean;
}
