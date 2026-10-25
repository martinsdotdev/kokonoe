<!-- The result in words once the game ends, and the way to a new one. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';

	interface Props {
		headline: string;
		detail: string;
		winner: Player | null;
		onrestart: () => void;
	}

	let { headline, detail, winner, onrestart }: Props = $props();

	const ink = $derived(
		winner === 'X' ? 'var(--x)' : winner === 'O' ? 'var(--o-text)' : 'var(--foreground)'
	);
</script>

<section
	aria-label="Result"
	class="flex flex-wrap items-center justify-between gap-5 rounded-xl border-2 border-foreground bg-card px-6 py-5"
	style:box-shadow="6px 6px 0 var(--muted)"
>
	<div class="flex min-w-0 items-center gap-5">
		<span
			class="flex size-16 flex-none items-center justify-center rounded-full border-2 font-display text-3xl font-bold"
			style:border-color={ink}
			style:color={ink}
			aria-hidden="true">{winner ?? '='}</span
		>
		<div class="min-w-0">
			<h2
				class="m-0 font-display text-[40px] leading-[1.05] font-semibold tracking-[-0.015em]"
				aria-live="polite"
			>
				{headline}
			</h2>
			<p class="mt-1 mb-0 text-muted-foreground">{detail}</p>
		</div>
	</div>
	<button
		type="button"
		class="min-h-12 rounded-md bg-primary px-5 font-semibold text-primary-foreground"
		onclick={onrestart}
	>
		New game
	</button>
</section>
