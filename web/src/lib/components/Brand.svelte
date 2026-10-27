<!-- The Kokonoe mark and name, linking Home: a hand-drawn grid with one X and one O. -->
<script lang="ts">
	import { resolve } from '$app/paths';
</script>

<a href={resolve('/')} class="flex items-center gap-3 text-foreground no-underline">
	<svg viewBox="0 0 40 40" class="size-10" aria-hidden="true">
		<path
			d="M14 4 L13 36 M27 4 L26 36 M4 14 L36 13 M4 27 L36 26"
			fill="none"
			stroke="var(--foreground)"
			stroke-width="2.6"
			stroke-linecap="round"
		/>
		<path
			d="M17 17 L23 23 M23 17 L17 23"
			fill="none"
			stroke="var(--x)"
			stroke-width="2.4"
			stroke-linecap="round"
		/>
		<circle cx="32" cy="32" r="3.2" fill="none" stroke="var(--o)" stroke-width="2" />
	</svg>
	<span class="font-display text-[34px] font-bold tracking-[-0.02em]">Kokonoe</span>
</a>
