export async function startViewTransition(update: () => void | Promise<void>): Promise<void> {
	if ('startViewTransition' in document) {
		await (document as Document & { startViewTransition: (cb: () => void | Promise<void>) => { finished: Promise<void> } })
			.startViewTransition(update).finished;
	} else {
		await update();
	}
}
