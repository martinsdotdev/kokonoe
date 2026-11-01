<!--
	The host's Room before a friend joins: its link to send, and Copy link. One status line says
	when the link is copied, and when the connection drops.
-->
<script lang="ts">
	import type { Player } from '#lib/engine.js';

	interface Props {
		/** The Room's link, `/r/<room>` on this site. */
		link: string;
		variant: string;
		you: Player;
		/** What keeps the friend from joining now, such as a dropped connection. */
		trouble: string | null;
	}

	let { link, variant, you, trouble }: Props = $props();

	/** What Copy link just did, for a moment. */
	let copied: string | null = $state(null);
	let field: HTMLInputElement | undefined = $state();
	let timer: ReturnType<typeof setTimeout> | undefined;

	const copy = async () => {
		try {
			await navigator.clipboard.writeText(link);
			copied = 'Link copied.';
		} catch {
			// Without the clipboard, the link is selected to copy by hand.
			field?.select();
			copied = 'The link is selected: copy it with Ctrl+C, or ⌘C on a Mac.';
		}
		clearTimeout(timer);
		timer = setTimeout(() => (copied = null), 4000);
	};

	$effect(() => () => clearTimeout(timer));
</script>

<section
	aria-labelledby="waiting"
	class="flex flex-col gap-5 rounded-[10px] border-[1.5px] border-foreground bg-card p-6 shadow-[6px_6px_0_var(--muted)]"
>
	<span class="font-mono text-[13px] tracking-[0.08em] text-muted-foreground uppercase">
		{variant} · You play {you}
	</span>
	<h1 id="waiting" class="m-0 font-display text-4xl leading-[1.1] font-semibold" tabindex="-1">
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
			Copy link
		</button>
	</div>
	<p class="m-0 min-h-6 text-sm font-medium" role="status" data-copied>
		{trouble ?? copied ?? ''}
	</p>
</section>
