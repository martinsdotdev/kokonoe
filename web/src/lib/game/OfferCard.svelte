<!--
	An offer waiting for an answer (spec §10.2): Accept and Decline for the player who answers it,
	or that it waits for the one who made it, and how long a takeback has left.
-->
<script lang="ts">
	import { seconds } from './listing.js';
	import { offerText, type PendingOffer } from './offers.js';

	interface Props {
		offer: PendingOffer;
		onanswer?: (accept: boolean) => void;
	}

	let { offer, onanswer }: Props = $props();

	/** When a takeback lapses: it's quick, so the card counts down. */
	const lapses = $derived(offer.kind === 'Takeback' ? offer.lapses : null);
	let now = $state(Date.now());
	$effect(() => {
		if (lapses === null) return;
		const ticking = setInterval(() => (now = Date.now()), 250);
		return () => clearInterval(ticking);
	});
	/** Whole seconds left to answer. */
	const left = $derived(lapses === null ? null : Math.max(0, Math.ceil((lapses - now) / 1000)));

	const button = 'min-h-11 flex-1 rounded-md border-[1.5px] px-4 font-semibold';
</script>

<section
	aria-label="Offer"
	class="flex flex-col gap-3 rounded-[10px] border-[1.5px] border-foreground bg-card p-4"
	data-offer={offer.kind}
>
	<p class="m-0 font-semibold">{offerText(offer)}</p>
	{#if left !== null}
		<p class="m-0 text-sm text-muted-foreground" data-left>
			{seconds(left)} left to answer
		</p>
	{/if}
	{#if !offer.mine && onanswer}
		<div class="flex flex-wrap gap-2.5">
			<button
				type="button"
				class="{button} border-foreground bg-primary text-primary-foreground"
				onclick={() => onanswer(true)}
			>
				Accept
			</button>
			<button
				type="button"
				class="{button} border-foreground bg-card"
				onclick={() => onanswer(false)}
			>
				Decline
			</button>
		</div>
	{/if}
</section>
