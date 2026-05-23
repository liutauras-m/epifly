import type { CapEntry } from '../CapabilityBrowser.svelte';

/**
 * Build a natural-language prompt for an "Invoke in current workspace" action.
 *
 * **PR 2.A change:** `forced_capability` now carries the structured routing hint
 * server-side, so this prompt no longer needs to name every tool for the semantic
 * router. A brief, natural instruction is sufficient — the LLM will see the pinned
 * tools and choose the right one.
 *
 * The verbose multi-tool path is kept as a fallback for non-button-initiated flows
 * (e.g. API callers that don't set `forced_capability`).
 */
export function buildInvocationPrompt(cap: CapEntry): string {
	const tools = cap.tools ?? [];

	if (tools.length === 1) {
		const t = tools[0];
		return t.description
			? `${t.description} (using ${cap.name})`
			: `Please use the ${cap.name} capability.`;
	}

	if (tools.length > 1) {
		// Natural fallback: let the LLM pick the right tool from the pinned set.
		return cap.description
			? `${cap.description}`
			: `Please use the ${cap.name} capability.`;
	}

	// No tools metadata — plain natural description.
	return cap.description ?? `Please use the ${cap.name} capability.`;
}
