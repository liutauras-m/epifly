export function tap(node: HTMLElement) {
	const platform = document.documentElement.dataset.platform ?? 'web';
	if (platform === 'android') {
		function onDown(e: PointerEvent) {
			const ripple = document.createElement('span');
			const rect = node.getBoundingClientRect();
			const size = Math.max(rect.width, rect.height) * 2;
			ripple.style.cssText = `
				position:absolute;pointer-events:none;border-radius:50%;
				width:${size}px;height:${size}px;
				left:${e.clientX - rect.left - size / 2}px;
				top:${e.clientY - rect.top - size / 2}px;
				background:var(--ember-soft);
				animation:ripple-expand 280ms ease-out forwards;
			`;
			node.style.position = node.style.position || 'relative';
			node.style.overflow = 'hidden';
			node.appendChild(ripple);
			setTimeout(() => ripple.remove(), 500);
		}
		node.addEventListener('pointerdown', onDown);
		return { destroy() { node.removeEventListener('pointerdown', onDown); } };
	}
	// Scale tap for iOS/macOS/Windows/web
	function onDown() {
		node.style.transition = 'transform 80ms var(--ease-out)';
		node.style.transform = 'scale(0.97)';
	}
	function onUp() {
		node.style.transition = 'transform 120ms var(--ease-out)';
		node.style.transform = '';
	}
	node.addEventListener('pointerdown', onDown);
	node.addEventListener('pointerup', onUp);
	node.addEventListener('pointercancel', onUp);
	return {
		destroy() {
			node.removeEventListener('pointerdown', onDown);
			node.removeEventListener('pointerup', onUp);
			node.removeEventListener('pointercancel', onUp);
		}
	};
}
