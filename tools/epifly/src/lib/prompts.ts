/**
 * @clack/prompts wrappers for epifly interactive wizard.
 */

import * as p from "@clack/prompts";

export { p };

/** Cancel guard — exits cleanly if user hits Ctrl-C at any prompt. */
export function checkCancel(value: unknown): asserts value is NonNullable<typeof value> {
  if (p.isCancel(value)) {
    p.cancel("Cancelled.");
    process.exit(0);
  }
}

/** Prompt for a non-empty string. */
export async function promptText(opts: {
  message: string;
  placeholder?: string;
  defaultValue?: string;
  validate?: (v: string) => string | undefined;
}): Promise<string> {
  const value = await p.text({
    message: opts.message,
    placeholder: opts.placeholder,
    defaultValue: opts.defaultValue,
    validate: opts.validate,
  });
  checkCancel(value);
  return value as string;
}

/** Prompt for a password (masked). */
export async function promptPassword(opts: {
  message: string;
  validate?: (v: string) => string | undefined;
}): Promise<string> {
  const value = await p.password({
    message: opts.message,
    validate: opts.validate,
  });
  checkCancel(value);
  return value as string;
}

/** Confirm yes/no. */
export async function promptConfirm(message: string, initialValue = false): Promise<boolean> {
  const value = await p.confirm({ message, initialValue });
  checkCancel(value);
  return value as boolean;
}

/** Select from a list. */
export async function promptSelect<T extends string>(opts: {
  message: string;
  options: Array<{ value: T; label: string; hint?: string }>;
  initialValue?: T;
}): Promise<T> {
  const value = await p.select({
    message: opts.message,
    options: opts.options as any,
    initialValue: opts.initialValue,
  });
  checkCancel(value);
  return value as T;
}
