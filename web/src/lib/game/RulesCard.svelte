<!-- The game's rules in plain words, as the design overview's game screens list them. -->
<script lang="ts">
	import { describeRules } from './rules.js';
	import type { Variant } from './rules.js';

	let { variant }: { variant: Variant } = $props();
</script>

<section aria-label="Rules" class="flex flex-col gap-2 px-0.5 py-1">
	<span class="font-mono text-xs tracking-[0.08em] text-muted-foreground">
		RULES · {variant.name.toUpperCase()}
	</span>
	<dl class="m-0 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
		{#each describeRules(variant.ruleset) as row (row.label)}
			<dt class="text-muted-foreground">{row.label}</dt>
			<dd class="m-0">{row.value}</dd>
		{/each}
	</dl>
</section>
