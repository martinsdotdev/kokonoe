<!--
	The header of the game screen and the Replay viewer: Kokonoe, the variant, what's shown and
	who it's against, and the way back to Home. Who it's against hides on a phone.
-->
<script lang="ts">
	import { resolve } from '$app/paths';

	interface Props {
		variant: string;
		/** What the page is, before who it's against: "Replay". */
		page?: string;
		/** Who the game is against: "Pass and play". */
		against: string;
		/** The link back to Home: "Leave game". */
		exit: string;
	}

	let { variant, page, against, exit }: Props = $props();
</script>

<header class="border-b-[1.5px] border-foreground bg-card">
	<div
		class="mx-auto flex max-w-[1320px] flex-wrap items-center justify-between gap-x-4 gap-y-1 px-4 py-2 sm:gap-y-4 sm:px-7 sm:py-3"
	>
		<div class="flex flex-wrap items-center gap-4">
			<a href={resolve('/')} class="font-display text-[26px] font-bold text-foreground no-underline"
				>Kokonoe</a
			>
			<span
				class="rounded border-[1.5px] border-foreground px-2.5 py-0.5 font-mono text-[13px]"
				data-variant
			>
				{variant}
			</span>
			<span class="text-muted-foreground"
				>{page ?? ''}<span class="max-sm:hidden">{page ? ` · ${against}` : against}</span></span
			>
		</div>
		<a
			href={resolve('/')}
			class="inline-flex min-h-11 items-center px-3.5 font-medium text-foreground"
		>
			{exit}
		</a>
	</div>
</header>
