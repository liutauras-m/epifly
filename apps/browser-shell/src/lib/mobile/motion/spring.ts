export interface SpringOpts {
	stiffness?: number;
	damping?: number;
}

export function prefersReducedMotion(): boolean {
	return typeof window !== 'undefined' &&
		window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

export function springAnimate(
	from: number, to: number,
	onUpdate: (v: number) => void,
	onDone?: () => void,
	opts: SpringOpts = {}
): () => void {
	if (prefersReducedMotion()) {
		onUpdate(to);
		onDone?.();
		return () => {};
	}
	const stiffness = opts.stiffness ?? 0.2;
	const damping = opts.damping ?? 0.8;
	let pos = from, vel = 0, raf = 0;
	function tick() {
		const force = (to - pos) * stiffness;
		vel = (vel + force) * damping;
		pos += vel;
		onUpdate(pos);
		if (Math.abs(to - pos) > 0.01 || Math.abs(vel) > 0.01) {
			raf = requestAnimationFrame(tick);
		} else {
			onUpdate(to);
			onDone?.();
		}
	}
	raf = requestAnimationFrame(tick);
	return () => cancelAnimationFrame(raf);
}
