<!--
	A stand-in for the game screen, which KOK-37 builds on this route. It starts a Standard game so
	the production build loads the engine, and the budget test counts the WASM.
-->
<script lang="ts">
	import { Game, loadEngine, newSeed, presets, type Status } from '#lib/engine.js';

	const turn = (status: Status) =>
		'ToMove' in status ? `${status.ToMove.player} to move` : 'Game over';

	const start = loadEngine().then(() => {
		const { name, ruleset } = presets()[0];
		return { name, game: new Game(ruleset, newSeed()) };
	});
</script>

<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 px-4">
	{#await start}
		<p class="text-muted-foreground">Loading the engine…</p>
	{:then { name, game }}
		<h1 class="font-display text-5xl font-semibold">{name}</h1>
		<p class="text-muted-foreground">{turn(game.status())}</p>
	{:catch}
		<p>The engine didn't load. Reload the page to try again.</p>
	{/await}
</main>
