/**
 * Apply an `InitialRoute` against the SDK to restore navigation state on mount.
 *
 * Lives in `packages/ui` and is consumed identically by `apps/web` (where the
 * route comes from `?ws=<id>`) and `apps/browser-shell` (where it can also
 * arrive via `conusai://?ws=<id>` deep links). The shared helper means both
 * apps get the same invalid-id behaviour: `onUnknown` is called when the ws
 * id either fails to resolve or returns `null` (PR 3.C.5).
 *
 * The routing primitives differ between apps (SvelteKit `goto` vs Tauri
 * webview history), so the helper takes callbacks instead of doing the
 * navigation itself.
 */

import type { ConusSdk } from '@conusai/sdk';
import type { InitialRoute } from './initialRoute.js';

export interface ApplyInitialRouteHandlers<TNode = unknown> {
  /** Called with the resolved workspace node when `route.ws` is valid. */
  onApplyNode: (node: TNode) => void;
  /** Called when `route.ws` is set but the SDK cannot resolve it (404 / network). */
  onUnknown: () => void;
}

export async function applyInitialRoute<TNode = unknown>(
  sdk: ConusSdk,
  route: InitialRoute,
  handlers: ApplyInitialRouteHandlers<TNode>,
): Promise<void> {
  if (!route.ws) return;
  try {
    // `sdk.workspaces.get` may not exist in older SDKs — fall back to `tree()`
    // + client-side find so the helper degrades gracefully.
    const wsApi = (sdk as unknown as { workspaces: { get?: (id: string) => Promise<{ data?: TNode | null; error?: unknown }>; tree?: () => Promise<{ data?: unknown; error?: unknown }> } }).workspaces;
    if (typeof wsApi.get === 'function') {
      const res = await wsApi.get(route.ws);
      if (res.error || !res.data) {
        handlers.onUnknown();
        return;
      }
      handlers.onApplyNode(res.data as TNode);
      return;
    }
    if (typeof wsApi.tree === 'function') {
      const tres = await wsApi.tree();
      if (tres.error) {
        handlers.onUnknown();
        return;
      }
      const list = tres.data as Array<{ id: string }> | null;
      const found = Array.isArray(list) ? list.find((n) => n.id === route.ws) : null;
      if (!found) {
        handlers.onUnknown();
        return;
      }
      handlers.onApplyNode(found as TNode);
      return;
    }
    handlers.onUnknown();
  } catch {
    handlers.onUnknown();
  }
}
