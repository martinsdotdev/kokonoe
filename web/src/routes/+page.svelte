<script lang="ts">
	import { probe } from '#lib/probe.js';
</script>

<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 px-4">
	<h1 class="font-display text-5xl font-semibold">Kokonoe</h1>
	<p class="text-muted-foreground">Ultimate Tic-Tac-Toe and its variants.</p>
	<p hidden>{probe.length}</p>
</main>
