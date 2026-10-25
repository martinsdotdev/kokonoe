<!-- One player's bar: their mark, their name, and whether it's their move. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';
	import Mark from './Mark.svelte';

	let { player, name, toMove }: { player: Player; name: string; toMove: boolean } = $props();

	const ink = $derived(player === 'X' ? 'var(--x)' : 'var(--o-text)');
</script>

<div
	class="flex items-center gap-3.5 rounded-[10px] bg-card p-4"
	style:border={toMove ? `2px solid ${ink}` : '1.5px solid var(--pencil)'}
	style:box-shadow={toMove ? `4px 4px 0 color-mix(in oklab, ${ink} 18%, transparent)` : 'none'}
	data-to-move={toMove || undefined}
>
	<Mark {player} class="size-8.5 shrink-0" />
	<div class="min-w-0 flex-1">
		<div class="font-semibold">{name}</div>
		<div
			class="text-sm"
			style:color={toMove ? ink : 'var(--muted-foreground)'}
			style:font-weight={toMove ? 600 : 400}
		>
			{toMove ? 'To move' : 'Waiting'}
		</div>
	</div>
	<span class="font-mono text-[13px] text-muted-foreground">{player}</span>
</div>
