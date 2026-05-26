/**
 * SSH helper — runs a remote command on the Dokploy host.
 * Used by `epifly wipe` to invoke wipe-volumes.sh on the server.
 */

import { spawnSync } from "node:child_process";

export interface SshOptions {
  /** SSH host, e.g. user@host or just host (if ~/.ssh/config has it). */
  host: string;
  /** Optional jump host for ProxyJump (-J), e.g. root@manager. */
  jumpHost?: string;
  /** Port, defaults to 22. */
  port?: number;
  /** Path to identity file (-i flag). */
  identityFile?: string;
}

/**
 * Run a command on the remote host over SSH and stream stdio to the terminal.
 * Throws if the command exits non-zero or SSH itself fails.
 */
export function runOverSsh(opts: SshOptions, command: string): void {
  const args: string[] = [];
  if (opts.jumpHost) args.push("-J", opts.jumpHost);
  if (opts.port) args.push("-p", String(opts.port));
  if (opts.identityFile) args.push("-i", opts.identityFile);
  // Batch mode — fail immediately if host key or auth prompts appear (don't hang).
  args.push("-o", "BatchMode=yes");
  args.push("-o", "StrictHostKeyChecking=accept-new");
  args.push(opts.host, command);

  const result = spawnSync("ssh", args, { stdio: "inherit" });
  if (result.error) throw new Error(`ssh failed to start: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(`Remote command exited with code ${result.status}`);
  }
}

/**
 * Run a command on the remote host and return stdout/stderr as text.
 * Useful for diagnostic commands where we need to parse or print output locally.
 */
export function runOverSshCapture(opts: SshOptions, command: string): string {
  const args: string[] = [];
  if (opts.jumpHost) args.push("-J", opts.jumpHost);
  if (opts.port) args.push("-p", String(opts.port));
  if (opts.identityFile) args.push("-i", opts.identityFile);
  args.push("-o", "BatchMode=yes");
  args.push("-o", "StrictHostKeyChecking=accept-new");
  args.push(opts.host, command);

  const result = spawnSync("ssh", args, { encoding: "utf8" });
  if (result.error) throw new Error(`ssh failed to start: ${result.error.message}`);
  if (result.status !== 0) {
    const stderr = (result.stderr ?? "").trim();
    throw new Error(stderr || `Remote command exited with code ${result.status}`);
  }
  return String(result.stdout ?? "");
}
