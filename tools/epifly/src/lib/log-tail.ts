/**
 * Tail compose deployment logs by polling composeStatus.
 */

import type { EpiflyConfig } from "./config.ts";
import { getCompose, listDeploymentQueue } from "./dokploy.ts";
import { info, warn } from "./ui.ts";

/**
 * Poll deployment status until done/error, streaming log lines to stdout.
 * Returns the final composeStatus string.
 */
export async function tailDeployLogs(
  cfg: EpiflyConfig,
  composeId: string,
  timeoutMs = 600_000
): Promise<string> {
  const start = Date.now();
  let lastStatus = "";
  let idleSince: number | null = null;
  let lastQueueState = "";

  while (Date.now() - start < timeoutMs) {
    const compose = await getCompose(cfg, composeId);
    const status: string = compose?.composeStatus ?? "unknown";

    if (status !== lastStatus) {
      info(`deploy status: ${status}`);
      lastStatus = status;
    }

    if (status === "done" || status === "error" || status === "failed") {
      return status;
    }

    if (status === "idle") {
      idleSince = idleSince ?? Date.now();
      const idleMs = Date.now() - idleSince;

      // When a compose stays idle for too long after triggerDeploy, inspect queue.
      if (idleMs >= 15_000) {
        try {
          const queue = await listDeploymentQueue(cfg);
          const pending = queue.filter((j: any) => j?.data?.composeId === composeId);
          if (pending.length > 0) {
            const latest = pending[0];
            const queueState = String(latest?.state ?? "unknown");
            if (queueState !== lastQueueState) {
              info(`deployment queue: ${queueState} (${pending.length} job(s))`);
              lastQueueState = queueState;
            }
            if (queueState === "waiting" && idleMs >= 60_000) {
              warn("Deployment is queued but not being processed by Dokploy worker.");
              return "queue_stalled";
            }
          }
        } catch {
          // Queue inspection is best-effort; keep polling compose status.
        }
      }
    } else {
      idleSince = null;
      lastQueueState = "";
    }

    await sleep(3000);
  }

  warn("Timed out waiting for deploy to complete.");
  return "timeout";
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
