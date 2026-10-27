<!--
	Home (spec §7.4): the way into a game. Continue and Recent games arrive with saved games
	(KOK-44), Online with M2, and Create custom with the editor (M3).
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import Brand from '#lib/components/Brand.svelte';
	import { DEFAULT_LEVEL, LEVELS, playQuery, type Level } from '#lib/game/launch.js';
	import Mark from '#lib/game/Mark.svelte';
	import { rulesetCode } from '#lib/game/rules.js';
	import { SHELF } from '#lib/game/shelf.js';

	let level: Level = $state(DEFAULT_LEVEL);

	const button =
		'inline-flex min-h-11 items-center justify-center rounded-md px-[18px] font-semibold no-underline';
	const card =
		'flex min-w-0 flex-[1_1_320px] flex-col gap-3.5 rounded-[10px] border-[1.5px] border-foreground bg-card p-6';
</script>

<div class="notebook min-h-svh">
	<header
		class="mx-auto flex max-w-[1240px] flex-wrap items-center justify-between gap-5 px-8 pt-7 pb-3"
	>
		<Brand />
		<nav aria-label="Main">
			<a href={resolve('/new-game')} class="px-3.5 py-2.5 font-medium text-foreground no-underline">
				New game
			</a>
		</nav>
	</header>

	<main class="mx-auto flex max-w-[1240px] flex-col gap-12 px-8 pt-4 pb-16">
		<section class="flex flex-col gap-4 py-6">
			<h1 class="m-0 font-display text-6xl leading-[1.02] font-semibold tracking-[-0.025em]">
				Ultimate <br />Tic-Tac-Toe
			</h1>
			<p class="m-0 max-w-[520px] text-[19px] text-muted-foreground">
				Nine boards inside one. Win three in a row. Every move you make decides which board your
				opponent plays next.
			</p>
			<a
				href={resolve('/new-game')}
				class="{button} mt-2 self-start bg-primary text-primary-foreground"
			>
				New game
			</a>
		</section>

		<section aria-labelledby="start" class="flex flex-col gap-4.5">
			<h2 id="start" class="m-0 font-display text-[32px] font-semibold">Start a game</h2>
			<div class="flex flex-wrap gap-5">
				<article class={card}>
					<h3 class="m-0 font-display text-2xl font-semibold">Against the computer</h3>
					<p class="m-0 text-muted-foreground">
						Pick a Level. The computer thinks on your device, so it works offline.
					</p>
					<fieldset class="m-0 min-w-0 border-0 p-0">
						<legend class="mb-2 p-0 text-sm font-medium">Level</legend>
						<div class="grid grid-cols-8 gap-1.5">
							{#each LEVELS as n (n)}
								<button
									type="button"
									aria-pressed={level === n}
									class="min-h-11 rounded-md border-[1.5px] font-mono text-[15px] aria-pressed:border-foreground aria-pressed:bg-primary aria-pressed:text-primary-foreground"
									onclick={() => (level = n)}>{n}</button
								>
							{/each}
						</div>
					</fieldset>
					<p class="m-0 text-sm text-muted-foreground">Level {level} of 8</p>
					<a
						href={resolve(`/new-game?vs=computer&level=${level}`)}
						class="{button} mt-auto self-start bg-primary text-primary-foreground"
					>
						Play the computer
					</a>
				</article>
				<article class={card}>
					<h3 class="m-0 font-display text-2xl font-semibold">Pass and play</h3>
					<p class="m-0 text-muted-foreground">
						Two players, one screen. Hand the device over after every move.
					</p>
					<div class="flex items-center gap-2.5">
						{#each [{ player: 'X', name: 'Player 1' }, { player: 'O', name: 'Player 2' }] as const as seat (seat.player)}
							<span
								class="inline-flex items-center gap-2 rounded-full border-[1.5px] px-3 py-1.5 font-semibold"
								style:border-color={seat.player === 'X' ? 'var(--x)' : 'var(--o-text)'}
								style:color={seat.player === 'X' ? 'var(--x)' : 'var(--o-text)'}
							>
								<Mark player={seat.player} class="size-5" />
								{seat.player} · {seat.name}
							</span>
						{/each}
					</div>
					<a
						href={resolve('/new-game?vs=hotseat')}
						class="{button} mt-auto self-start bg-primary text-primary-foreground"
					>
						Start pass and play
					</a>
				</article>
			</div>
		</section>

		<section aria-labelledby="variants" class="flex flex-col gap-4.5">
			<h2 id="variants" class="m-0 font-display text-[32px] font-semibold">Variants</h2>
			<div class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-4">
				{#each SHELF as preset (preset.name)}
					<article
						aria-label={preset.name}
						class="flex flex-col gap-2.5 rounded-[10px] border-[1.5px] border-foreground bg-card p-4.5"
					>
						<h3 class="m-0 font-display text-[21px] font-semibold">{preset.name}</h3>
						<p class="m-0 text-sm text-muted-foreground">{preset.blurb}</p>
						<div class="mt-auto flex flex-wrap gap-1.5">
							{#each preset.chips as chip (chip)}
								<span class="rounded border px-2 py-0.5 font-mono text-xs text-muted-foreground"
									>{chip}</span
								>
							{/each}
						</div>
						{#if preset.playable}
							<a
								href={resolve(
									`/play${playQuery({ code: rulesetCode(preset.ruleset), opponent: { kind: 'hotseat' } })}`
								)}
								class="{button} self-start border-[1.5px] border-foreground text-foreground"
								aria-label={`Play ${preset.name}, pass and play`}
							>
								Play
							</a>
						{:else}
							<span class="font-mono text-xs tracking-[0.08em] text-muted-foreground"
								>ARRIVES SOON</span
							>
						{/if}
					</article>
				{/each}
			</div>
		</section>
	</main>
</div>

<style>
	/* Notebook paper: faint ruled lines in the X ink. */
	.notebook {
		background-image: repeating-linear-gradient(
			0deg,
			color-mix(in oklab, var(--x) 7%, transparent) 0 1px,
			transparent 1px 36px
		);
	}
</style>
