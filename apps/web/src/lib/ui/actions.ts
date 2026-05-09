/**
 * Svelte action: auto-grow a textarea to fit its content, up to maxHeight px.
 * Usage: <textarea use:autoGrow />
 */
export function autoGrow(node: HTMLTextAreaElement, maxHeight = 240) {
	function resize() {
		node.style.height = 'auto';
		node.style.height = Math.min(node.scrollHeight, maxHeight) + 'px';
	}
	node.addEventListener('input', resize);
	// Also resize when value is set programmatically (e.g. bind:value clearing)
	const observer = new MutationObserver(resize);
	observer.observe(node, { attributes: true, attributeFilter: ['value'] });
	return {
		destroy() {
			node.removeEventListener('input', resize);
			observer.disconnect();
		}
	};
}
