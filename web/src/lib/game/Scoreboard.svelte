<!--
	The count scoreboard (spec §7.4) for games decided by won boards: "X 3 · O 2 · first to 5", or
	"most boards wins" when only the MostWon fallback counts.
-->
<script lang="ts">
	import type { Player, Ruleset } from '#lib/engine.js';

	let { won, ruleset }: { won: Record<Player, number>; ruleset: Ruleset } = $props();

	const target = $derived(
		ruleset.objective === 'Line' ? 'most boards wins' : `first to ${ruleset.objective.FirstTo}`
	);
</script>

<section
	aria-label="Score"
	class="flex flex-col gap-1 rounded-[10px] border-[1.5px] border-foreground bg-card p-4"
>
	<span class="font-mono text-xs tracking-[0.08em] text-muted-foreground">BOARDS WON</span>
	<p class="m-0 font-display text-2xl font-semibold" data-score>
		<span style:color="var(--x)">X {won.X}</span> ·
		<span style:color="var(--o-text)">O {won.O}</span> · {target}
	</p>
</section>
