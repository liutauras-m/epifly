// Injected into <head> by both apps via %sveltekit.head% to prevent FOUC.
// Keep this pure JS — no ES module syntax; it runs before any framework loads.
export const THEME_SCRIPT = `(function(){var t=localStorage.getItem('conusai-theme')||'paper';document.documentElement.setAttribute('data-theme',t);})();`;
