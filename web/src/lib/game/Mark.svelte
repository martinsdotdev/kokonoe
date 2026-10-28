<!--
	An X or an O as drawn in pen: the design overview's glyphs, in the player's ink. A mark a
	Random opening dealt is drawn in pencil grey, fainter than the moves players made.
-->
<script lang="ts">
	import type { Player } from '#lib/engine.js';

	interface Props {
		player: Player;
		/** Dealt by a Random opening rather than played. */
		dealt?: boolean;
		class?: string;
	}

	let { player, dealt = false, class: className = '' }: Props = $props();
</script>

<svg viewBox="0 0 24 24" class={className} aria-hidden="true">
	{#if player === 'X'}
		<path
			d="M6.5 6.8 L17.4 17.2 M17.3 6.6 L6.7 17.4"
			fill="none"
			stroke={dealt ? 'var(--dealt)' : 'var(--x)'}
			stroke-width="2.4"
			stroke-linecap="round"
		/>
	{:else}
		<circle
			cx="12"
			cy="12"
			r="6.2"
			fill="none"
			stroke={dealt ? 'var(--dealt)' : 'var(--o)'}
			stroke-width="2.4"
		/>
	{/if}
</svg>
