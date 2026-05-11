/**
 * Wrapper around the View Transitions API.
 * Falls through to a plain `await update()` when the browser does not
 * support `document.startViewTransition`, preserving the same call site.
 */
export async function startViewTransition(
  update: () => void | Promise<void>,
): Promise<void> {
  if (typeof document !== "undefined" && "startViewTransition" in document) {
    await (
      document as Document & {
        startViewTransition: (
          cb: () => void | Promise<void>,
        ) => { finished: Promise<void> };
      }
    )
      .startViewTransition(update)
      .finished;
  } else {
    await update();
  }
}
