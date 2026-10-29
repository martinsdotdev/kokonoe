<!--
	Import a game (spec §14): a `.utt` or JSON file, pasted, dropped or chosen, read and checked
	here by the engine and opened in the Replay viewer. It never leaves the device, and isn't
	saved: the viewer's history state carries it.
-->
<script lang="ts">
	import { tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Brand from '#lib/components/Brand.svelte';
	import { loadEngine, parseRulesetCode } from '#lib/engine.js';
	import { openFile } from '#lib/game/fileimport.js';
	import { FileProblem, fromFile, parseFile, variantName } from '#lib/game/fileparse.js';
	import { ENGINE, explain, failed } from '#lib/game/problem.js';

	/** No game file comes near this: a long Deep game is a few kilobytes. */
	const MAX_BYTES = 1_000_000;

	let text = $state('');
	let problem: string | null = $state(null);
	/** A file is being dragged over the form. */
	let over = $state(false);
	let box: HTMLTextAreaElement;

	// Load the engine while the player finds the file.
	const engine = loadEngine();

	/** Says what went wrong, and puts the player back in the box to fix it. */
	const refuse = async (error: unknown) => {
		// Cleared first, so the same words twice are still announced twice.
		problem = null;
		await tick();
		problem = explain(error, 'The file couldn’t be imported.');
		box.focus();
	};

	const open = async () => {
		try {
			await engine.catch(failed(ENGINE));
			const parsed = parseFile(text, parseRulesetCode);
			const { resigned } = openFile(parsed);
			const imported = fromFile(parsed, variantName(parsed), resigned);
			await goto(resolve('/replay'), { state: { imported }, persistState: true });
		} catch (error) {
			await refuse(error);
		}
	};

	/** Reads a dropped or chosen file into the box, then opens it. */
	const read = async (file: File | undefined) => {
		if (!file) return;
		try {
			if (file.size > MAX_BYTES) throw new FileProblem('That file is too large to be a game.');
			text = await file.text().catch(() => {
				throw new FileProblem('That file couldn’t be read.');
			});
		} catch (error) {
			await refuse(error);
			return;
		}
		await open();
	};

	/** Files dragged in are the form's; text dragged in is the box's own business. */
	const carriesFiles = (event: DragEvent) => event.dataTransfer?.types.includes('Files') ?? false;

	const ondragover = (event: DragEvent) => {
		if (!carriesFiles(event)) return;
		event.preventDefault();
		over = true;
	};

	const ondragleave = (event: DragEvent & { currentTarget: HTMLElement }) => {
		// Crossing into the box or a button inside the form isn't leaving it.
		if (!event.currentTarget.contains(event.relatedTarget as Node | null)) over = false;
	};

	const ondrop = (event: DragEvent) => {
		if (!carriesFiles(event)) return;
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
			{ondragover}
			{ondragleave}
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
				bind:this={box}
				bind:value={text}
				rows="10"
				spellcheck="false"
				aria-invalid={problem ? 'true' : undefined}
				aria-describedby={problem ? 'game-problem' : undefined}
				class="min-h-40 rounded-md border-[1.5px] border-foreground bg-background p-3 font-mono text-sm"
			></textarea>
			<p class="m-0 text-sm text-muted-foreground">You can also drop a file on this box.</p>
			<div class="flex flex-wrap items-center gap-3">
				<label class="{button} bg-card focus-within:outline-3 focus-within:outline-ring">
					Choose a file
					<input
						type="file"
						accept=".utt,.json,text/plain,application/json"
						class="sr-only"
						onchange={(event) => {
							const input = event.currentTarget;
							const file = input.files?.[0];
							// Cleared, so choosing the same file again, once fixed, reads it again.
							input.value = '';
							read(file);
						}}
					/>
				</label>
				<button type="submit" class="{button} bg-primary text-primary-foreground">
					Open the replay
				</button>
			</div>
			<p
				id="game-problem"
				class="m-0 font-medium text-[var(--o-text)]"
				aria-live="polite"
				data-problem
			>
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
