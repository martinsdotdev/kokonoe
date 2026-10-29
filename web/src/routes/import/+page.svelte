<!--
	Import a game (spec §14): a `.utt` or JSON file, pasted, dropped or chosen, read and checked
	here by the engine and opened in the Replay viewer. It never leaves the device, and isn't
	saved: the viewer's history state carries it.
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Brand from '#lib/components/Brand.svelte';
	import { loadEngine, parseRulesetCode } from '#lib/engine.js';
	import { FileProblem, openFile, parseFile, variantName } from '#lib/game/fileimport.js';
	import { ENGINE } from '#lib/game/problem.js';
	import { fromFile } from '#lib/game/replay.svelte.js';

	/** No game file comes near this: a long Deep game is a few kilobytes. */
	const LARGEST = 1_000_000;

	let text = $state('');
	let problem: string | null = $state(null);
	/** A file is being dragged over the drop zone. */
	let over = $state(false);

	// Load the engine while the player finds the file.
	const engine = loadEngine();

	const open = async () => {
		problem = null;
		try {
			await engine;
		} catch (error) {
			console.error(ENGINE, error);
			problem = ENGINE;
			return;
		}
		try {
			const parsed = parseFile(text, parseRulesetCode);
			const { resigned } = openFile(parsed);
			const imported = fromFile(parsed, variantName(parsed), resigned);
			await goto(resolve('/replay'), { state: { imported }, persistState: true });
		} catch (error) {
			if (error instanceof FileProblem) {
				problem = error.message;
				return;
			}
			console.error('The file couldn’t be imported.', error);
			problem = 'Something went wrong. Reload the page to try again.';
		}
	};

	/** Reads a dropped or chosen file into the box, then opens it. */
	const read = async (file: File | undefined) => {
		if (!file) return;
		if (file.size > LARGEST) {
			problem = 'That file is too large to be a game.';
			return;
		}
		text = await file.text();
		await open();
	};

	const ondrop = (event: DragEvent) => {
		event.preventDefault();
		over = false;
		read(event.dataTransfer?.files[0]);
	};

	const button =
		'inline-flex min-h-11 cursor-pointer items-center rounded-md border-[1.5px] border-foreground px-4 font-semibold';
</script>

<svelte:head>
	<title>Import a game · Kokonoe</title>
</svelte:head>

<div class="min-h-svh">
	<header
		class="mx-auto flex max-w-[1240px] flex-wrap items-center justify-between gap-4 px-8 py-6"
	>
		<Brand />
		<a
			href={resolve('/')}
			class="inline-flex min-h-11 items-center px-3.5 font-medium text-foreground"
		>
			Cancel
		</a>
	</header>

	<main class="mx-auto flex max-w-[760px] flex-col gap-6 px-8 pb-12">
		<h1 class="m-0 font-display text-5xl font-semibold tracking-[-0.02em]">Import a game</h1>
		<p class="m-0 text-muted-foreground">
			A game exported from Kokonoe, as a .utt or JSON file. It opens in the Replay viewer, and stays
			on this device.
		</p>

		<form
			class="flex flex-col gap-4 rounded-[10px] border-[1.5px] border-foreground bg-card p-6"
			class:over
			ondragover={(event) => {
				event.preventDefault();
				over = true;
			}}
			ondragleave={() => (over = false)}
			{ondrop}
			onsubmit={(event) => {
				event.preventDefault();
				open();
			}}
			data-drop
		>
			<label class="font-semibold" for="game">Paste a .utt or JSON game</label>
			<textarea
				id="game"
				bind:value={text}
				rows="10"
				spellcheck="false"
				class="min-h-40 rounded-md border-[1.5px] border-foreground bg-background p-3 font-mono text-sm"
			></textarea>
			<p class="m-0 text-sm text-muted-foreground">or drop a file here, or</p>
			<div class="flex flex-wrap items-center gap-3">
				<label class="{button} bg-card focus-within:outline-3 focus-within:outline-ring">
					Choose a file
					<input
						type="file"
						accept=".utt,.json,text/plain,application/json"
						class="sr-only"
						onchange={(event) => read(event.currentTarget.files?.[0])}
					/>
				</label>
				<button type="submit" class="{button} bg-primary text-primary-foreground">
					Open the replay
				</button>
			</div>
			<p class="m-0 font-medium text-[var(--o-text)]" aria-live="polite" data-problem>
				{problem ?? ''}
			</p>
		</form>
	</main>
</div>

<style>
	form.over {
		outline: 3px dashed var(--foreground);
		outline-offset: 4px;
	}
</style>
