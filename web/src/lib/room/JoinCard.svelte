<!--
	What a Room's link shows before its holder joins (spec §10.1): who invites them to what, under
	which rules, and Accept or Decline.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { RoomCard } from '#lib/engine.js';
	import RulesCard from '#lib/game/RulesCard.svelte';
	import { presetByCode } from '#lib/game/shelf.js';

	interface Props {
		card: RoomCard;
		onaccept: () => void;
	}

	let { card, onaccept }: Props = $props();

	const preset = $derived(presetByCode(card.rules));
	const chip = 'rounded-full border-[1.5px] border-foreground px-3 py-1 font-mono text-sm';
</script>

<section
	aria-labelledby="invitation"
	class="flex flex-col gap-5 rounded-[10px] border-[1.5px] border-foreground bg-card p-6 shadow-[6px_6px_0_var(--muted)]"
>
	<span class="font-mono text-[13px] tracking-[0.08em] text-muted-foreground">INVITATION</span>
	<h1 id="invitation" class="m-0 font-display text-4xl leading-[1.1] font-semibold" tabindex="-1">
		<span data-host>{card.host}</span> invites you to a game of {card.variant}
	</h1>
	<!-- role="list", since Safari drops a list's role with its markers. -->
	<ul aria-label="The game" role="list" class="m-0 flex list-none flex-wrap gap-2 p-0">
		<li class={chip}>{preset ? 'Preset' : 'Custom variant'}</li>
		<li class={chip}>Casual</li>
		<li class={chip}>Untimed</li>
	</ul>
	{#if preset}
		<RulesCard variant={{ name: preset.name, ruleset: preset.ruleset }} />
	{/if}
	<div class="flex flex-wrap gap-3">
		<button
			type="button"
			class="min-h-13 flex-1 rounded-md bg-primary px-6 text-[17px] font-semibold text-primary-foreground"
			onclick={onaccept}
		>
			Accept
		</button>
		<a
			href={resolve('/')}
			class="inline-flex min-h-13 flex-1 items-center justify-center rounded-md border-[1.5px] border-foreground px-6 font-semibold text-foreground no-underline"
		>
			Decline
		</a>
	</div>
</section>
