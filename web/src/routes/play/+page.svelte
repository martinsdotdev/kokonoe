<!--
	The game screen. Its URL names the game (see launch.ts): a new one by its Ruleset code and
	opponent, or a saved one by its id. Once a game is saved the address becomes its id, so a reload
	resumes it.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { loadEngine, parseRulesetCode, type Ruleset } from '#lib/engine.js';
	import GameScreen from '#lib/game/GameScreen.svelte';
	import { gameQuery, playQuery, readLaunch } from '#lib/game/launch.js';
	import { rulesetCode } from '#lib/game/rules.js';
	import { loadGame } from '#lib/game/saved.js';
	import { UnplayableGame } from '#lib/game/record.js';
	import { OfflineGame, type Variant } from '#lib/game/session.svelte.js';
	import { presetByCode } from '#lib/game/shelf.js';

	let session: OfflineGame | null = $state(null);
	let problem: string | null = $state(null);
	/** The page was left, perhaps before the engine loaded. */
	let left = false;

	/** A failure the page explains to the player, rather than only logging it. */
	class Problem extends Error {}

	/** Throws `message` for the page to show, with the cause logged where a developer can see it. */
	const failed = (message: string) => (error: unknown) => {
		console.error(message, error);
		throw new Problem(message);
	};

	const ENGINE = 'The engine didn’t load. Reload the page to try again.';

	/**
	 * The game the address names: a saved one by its id, or a new one by its launch query. Null
	 * once the page has been left, since a computer on X would start a search nothing stops.
	 */
	const open = async (): Promise<OfflineGame | null> => {
		const id = page.url.searchParams.get('game');
		if (id !== null) {
			const [saved] = await Promise.all([
				loadGame(id).catch(failed('Saved games can’t be read in this browser.')),
				loadEngine().catch(failed(ENGINE))
			]);
			if (!saved) throw new Problem('This game isn’t saved on this device.');
			try {
				return left ? null : OfflineGame.resume(saved);
			} catch (error) {
				if (error instanceof UnplayableGame) {
					throw new Problem(`This game can’t be resumed. ${error.message}`);
				}
				throw error;
			}
		}
		const launch = readLaunch(page.url.searchParams);
		if ('error' in launch) throw new Problem(launch.error);
		await loadEngine().catch(failed(ENGINE));
		const variant = variantOf(launch.code);
		return left ? null : new OfflineGame(variant, launch.opponent);
	};

	/** The variant a Ruleset code names: a Preset by its name, or any other Ruleset. */
	const variantOf = (code: string): Variant => {
		let ruleset: Ruleset;
		try {
			ruleset = parseRulesetCode(code);
		} catch (error) {
			throw new Problem(`Kokonoe doesn’t know the variant “${code}”: ${(error as Error).message}.`);
		}
		const preset = presetByCode(code);
		return { name: preset?.name ?? 'Custom variant', ruleset };
	};

	open().then(
		(game) => (session = game),
		(error: unknown) => {
			if (!(error instanceof Problem)) console.error('The game couldn’t start.', error);
			problem =
				error instanceof Problem
					? error.message
					: 'Something went wrong. Reload the page to try again.';
		}
	);

	// The address follows the game: its id once saved, or the new game a restart begins.
	$effect(() => {
		if (!session) return;
		const query = session.stored
			? gameQuery(session.id)
			: playQuery({ code: rulesetCode(session.variant.ruleset), opponent: session.opponent });
		if (location.search !== query) replaceState(resolve(`/play${query}`), {});
	});

	onDestroy(() => {
		left = true;
		session?.dispose();
	});
</script>

{#if session}
	<GameScreen {session} />
{:else}
	<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 px-4">
		{#if problem}
			<h1 class="font-display text-3xl font-semibold">This game can’t start</h1>
			<p class="text-muted-foreground">{problem}</p>
			<a href={resolve('/new-game')} class="font-semibold text-foreground">Choose a new game</a>
		{:else}
			<p class="text-muted-foreground">Loading the engine…</p>
		{/if}
	</main>
{/if}
