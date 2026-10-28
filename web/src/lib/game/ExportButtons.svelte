<!--
	A game as a file (spec §14): a `.utt` or JSON download, or the `.utt` text copied, to paste
	somewhere else. Nothing leaves the device but through the player's own hands.
-->
<script lang="ts">
	import Braces from '@lucide/svelte/icons/braces';
	import Copy from '@lucide/svelte/icons/copy';
	import FileDown from '@lucide/svelte/icons/file-down';
	import { fileName, writeJson, writeUtt, type GameFile } from './gamefile.js';

	let { file }: { file: GameFile } = $props();

	/** How long "Copied" stays. */
	const SAID_MS = 2000;

	/** What the last copy did, said until `SAID_MS` passes. */
	let said: string | null = $state(null);
	let timer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => () => clearTimeout(timer));

	const download = (text: string, extension: 'utt' | 'json', type: string) => {
		const url = URL.createObjectURL(new Blob([text], { type }));
		const link = document.createElement('a');
		link.href = url;
		link.download = fileName(file, extension);
		link.click();
		// Some browsers start the download after the click returns.
		setTimeout(() => URL.revokeObjectURL(url));
	};

	const copy = async () => {
		clearTimeout(timer);
		try {
			await navigator.clipboard.writeText(writeUtt(file));
			said = 'Copied';
		} catch {
			said = 'Couldn’t copy';
		}
		timer = setTimeout(() => (said = null), SAID_MS);
	};

	const button =
		'inline-flex min-h-11 items-center gap-2 rounded-md border-[1.5px] border-foreground bg-card px-3.5 font-semibold';
</script>

<div class="flex flex-wrap items-center gap-2" role="group" aria-label="Export">
	<button
		type="button"
		class={button}
		aria-label="Export as .utt"
		onclick={() => download(writeUtt(file), 'utt', 'text/plain;charset=utf-8')}
	>
		<FileDown class="size-5" aria-hidden="true" />.utt
	</button>
	<button
		type="button"
		class={button}
		aria-label="Export as JSON"
		onclick={() => download(writeJson(file), 'json', 'application/json')}
	>
		<Braces class="size-5" aria-hidden="true" />JSON
	</button>
	<button type="button" class={button} onclick={copy}>
		<Copy class="size-5" aria-hidden="true" />Copy .utt
	</button>
	<span role="status" class="min-w-24 text-sm text-muted-foreground">{said ?? ''}</span>
</div>
