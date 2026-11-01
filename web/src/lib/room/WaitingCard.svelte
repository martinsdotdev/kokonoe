<!-- The host's Room before a friend joins: its link to send, and Copy link. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';

	interface Props {
		/** The Room's link, `/r/<room>` on this site. */
		link: string;
		variant: string;
		you: Player;
	}

	let { link, variant, you }: Props = $props();

	/** The link was just copied, so the button says so for a moment. */
	let copied = $state(false);
	let field: HTMLInputElement | undefined = $state();
	let timer: ReturnType<typeof setTimeout> | undefined;

	const copy = async () => {
		try {
			await navigator.clipboard.writeText(link);
		} catch {
			// Without the clipboard, select the link so it can be copied by hand.
			field?.select();
			return;
		}
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 2000);
	};
</script>

<section
	aria-labelledby="waiting"
	class="flex flex-col gap-5 rounded-[10px] border-[1.5px] border-foreground bg-card p-6 shadow-[6px_6px_0_var(--muted)]"
>
	<span class="font-mono text-[13px] tracking-[0.08em] text-muted-foreground">
		{variant.toUpperCase()} · YOU PLAY {you}
	</span>
	<h1 id="waiting" class="m-0 font-display text-4xl leading-[1.1] font-semibold">
		Waiting for your friend
	</h1>
	<p class="m-0 text-muted-foreground">
		Send them this link. The game starts as soon as they join.
	</p>
	<div class="flex flex-wrap gap-3">
		<label class="flex min-w-0 flex-[1_1_260px] flex-col gap-1 text-sm font-medium">
			Invite link
			<input
				bind:this={field}
				readonly
				value={link}
				class="min-h-12 rounded-md border-[1.5px] border-foreground bg-background px-3 font-mono text-[15px]"
				onfocus={(event) => event.currentTarget.select()}
			/>
		</label>
		<button
			type="button"
			class="min-h-12 flex-[0_1_180px] self-end rounded-md bg-primary px-6 font-semibold text-primary-foreground"
			onclick={copy}
		>
			{copied ? 'Copied' : 'Copy link'}
		</button>
	</div>
	<p class="sr-only" aria-live="polite">{copied ? 'Link copied' : ''}</p>
</section>
