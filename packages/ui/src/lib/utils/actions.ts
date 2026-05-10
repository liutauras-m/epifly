export function autoGrow(node: HTMLTextAreaElement, maxHeight = 240) {
  function resize() {
    node.style.height = 'auto';
    node.style.height = Math.min(node.scrollHeight, maxHeight) + 'px';
  }
  node.addEventListener('input', resize);
  const observer = new MutationObserver(resize);
  observer.observe(node, { attributes: true, attributeFilter: ['value'] });
  return {
    destroy() {
      node.removeEventListener('input', resize);
      observer.disconnect();
    }
  };
}
