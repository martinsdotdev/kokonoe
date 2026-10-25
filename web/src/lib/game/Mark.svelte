<!-- An X or an O as drawn in pen: the design overview's glyphs, in the player's ink. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';

	let { player, class: className = '' }: { player: Player; class?: string } = $props();
</script>

<svg viewBox="0 0 24 24" class={className} aria-hidden="true">
	{#if player === 'X'}
		<path
			d="M6.5 6.8 L17.4 17.2 M17.3 6.6 L6.7 17.4"
			fill="none"
			stroke="var(--x)"
			stroke-width="2.4"
			stroke-linecap="round"
		/>
	{:else}
		<circle cx="12" cy="12" r="6.2" fill="none" stroke="var(--o)" stroke-width="2.4" />
	{/if}
</svg>
