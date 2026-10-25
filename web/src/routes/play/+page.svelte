<!-- The game screen: a Standard game, passed and played on one device. -->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { loadEngine } from '#lib/engine.js';
	import GameScreen from '#lib/game/GameScreen.svelte';
	import { HotSeat } from '#lib/game/session.svelte.js';

	let session: HotSeat | null = $state(null);
	let failed = $state(false);

	loadEngine().then(
		() => (session = new HotSeat()),
		(error: unknown) => {
			// The page only says it failed, so say why where a developer can see it.
			console.error('The engine failed to load.', error);
			failed = true;
		}
	);
	onDestroy(() => session?.dispose());
</script>

{#if session}
	<GameScreen {session} />
{:else}
	<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 px-4">
		<p class="text-muted-foreground">
			{failed ? 'The engine didn’t load. Reload the page to try again.' : 'Loading the engine…'}
		</p>
	</main>
{/if}
