<!--
	A Claim against a player who left the Room (spec §11), once it's due: the win or a draw, or
	keep waiting, which is doing nothing. A Claim helps only its claimant, so it asks no
	confirmation. Its buttons stay focusable while it can't be made.
-->
<script lang="ts">
	import type { ClaimKind } from '#lib/engine.js';
	import { claimText, type Claim } from './presence.js';

	let { claim }: { claim: Claim } = $props();

	const CLAIMS: [ClaimKind, string][] = [
		['Win', 'Claim the win'],
		['Draw', 'Claim a draw']
	];

	const button =
		'min-h-11 flex-1 rounded-md border-[1.5px] border-foreground px-4 font-semibold aria-disabled:opacity-40';
</script>

<section
	aria-label="Claim"
	class="flex flex-col gap-3 rounded-[10px] border-[1.5px] border-foreground bg-card p-4"
	data-claim
>
	<p class="m-0 font-semibold">{claimText(claim.other)}</p>
	<div class="flex flex-wrap gap-2.5">
		{#each CLAIMS as [kind, label] (kind)}
			<button
				type="button"
				class="{button} {kind === 'Win' ? 'bg-primary text-primary-foreground' : 'bg-card'}"
				aria-disabled={!claim.enabled}
				onclick={() => claim.enabled && claim.run(kind)}
			>
				{label}
			</button>
		{/each}
	</div>
</section>
