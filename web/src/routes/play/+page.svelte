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
	import { rulesetCode, type Variant } from '#lib/game/rules.js';
	import PageProblem from '#lib/game/PageProblem.svelte';
	import { ENGINE, explain, failed, loadSaved, Problem } from '#lib/game/problem.js';
	import { UnplayableGame } from '#lib/game/record.js';
	import { OfflineGame } from '#lib/game/session.svelte.js';
	import { presetByCode } from '#lib/game/shelf.js';

	let session: OfflineGame | null = $state(null);
	let problem: string | null = $state(null);
	/** The page was left, perhaps before the engine loaded. */
	let left = false;

	/**
	 * The game the address names: a saved one by its id, or a new one by its launch query. Null
	 * once the page has been left, since a computer on X would start a search nothing stops.
	 */
	const open = async (): Promise<OfflineGame | null> => {
		const id = page.url.searchParams.get('game');
		if (id !== null) {
			const saved = await loadSaved(id);
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
		(error: unknown) => (problem = explain(error, 'The game couldn’t start.'))
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
	<PageProblem {problem} title="This game can’t start" to="/new-game" link="Choose a new game" />
{/if}
