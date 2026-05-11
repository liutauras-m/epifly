export function stagger(node: HTMLElement, opts: { delay?: number } = {}) {
	const delay = opts.delay ?? 40;
	function apply() {
		Array.from(node.children).forEach((child, i) => {
			(child as HTMLElement).style.setProperty('--stagger-i', String(i));
			(child as HTMLElement).style.setProperty('--stagger-delay', `${i * delay}ms`);
			(child as HTMLElement).style.animationDelay = `${i * delay}ms`;
		});
	}
	apply();
	const mo = new MutationObserver(apply);
	mo.observe(node, { childList: true });
	return { destroy() { mo.disconnect(); } };
}
