<!--
	The New game sheet (spec §7.4): pick the opponent and the variant, read the game's rules, start.
	Home's cards prefill it through `vs`, `level` and `rules` in the query. A friend plays in an
	invite Room, made here; Quick match arrives later in M2, and saved Custom variants in M3.
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Player } from '#lib/engine.js';
	import { startSession } from '#lib/account.js';
	import Brand from '#lib/components/Brand.svelte';
	import { DEFAULT_LEVEL, LEVELS, playQuery, type Level, type Opponent } from '#lib/game/launch.js';
	import { describeRules, rulesetCode } from '#lib/game/rules.js';
	import { SHELF, presetByCode } from '#lib/game/shelf.js';
	import { createRoom } from '#lib/room/api.js';

	const params = page.url.searchParams;
	const asked = LEVELS.find((n) => String(n) === params.get('level'));
	const prefill = presetByCode(params.get('rules') ?? 'std');

	type Kind = Opponent['kind'] | 'friend';
	/** How the sheet speaks of each opponent: the summary's line, the heading, the button. */
	const KINDS: Record<Kind, { opponent: (level: Level) => string; title: string; start: string }> =
		{
			computer: {
				opponent: (level) => `Computer, Level ${level}`,
				title: 'against the computer',
				start: 'Start game'
			},
			hotseat: {
				opponent: () => 'Two players, one screen',
				title: 'pass and play',
				start: 'Start game'
			},
			friend: {
				opponent: () => 'A friend, by invite link',
				title: 'with a friend',
				start: 'Create invite Room'
			}
		};
	const asks = params.get('vs');
	let kind: Kind = $state(asks === 'hotseat' || asks === 'friend' ? asks : 'computer');
	let level: Level = $state(asked ?? DEFAULT_LEVEL);
	/** The seat against the computer, X unless chosen; in an invite Room, Random (spec §10.1). */
	let seats = $state<Record<'computer' | 'friend', Player | 'Random'>>({
		computer: 'X',
		friend: 'Random'
	});
	const seatOf = $derived(kind === 'friend' ? 'friend' : 'computer');
	const seat = $derived(seats[seatOf]);
	/** An invite Room lets its players take back a move, off unless chosen (spec §10.2). */
	let takebacks = $state(false);
	let chosen = $state(prefill?.name ?? 'Standard');
	/** An invite Room is being made, or why it couldn't be. */
	let creating = $state(false);
	let failure: string | null = $state(null);

	const preset = $derived(SHELF.find((p) => p.name === chosen) ?? SHELF[0]);
	const summary = $derived([
		{ label: 'Opponent', value: KINDS[kind].opponent(level) },
		...(kind === 'hotseat'
			? []
			: [
					{
						label: 'You play',
						value:
							seat === 'Random'
								? `X or O, drawn when ${kind === 'friend' ? 'the Room is made' : 'you start'}`
								: seat === 'X'
									? 'X, moving first'
									: 'O, moving second'
					}
				]),
		...(kind === 'friend'
			? [
					{ label: 'Game', value: 'Casual, untimed' },
					{
						label: 'Takebacks',
						value: takebacks ? 'Allowed, if the other player agrees' : 'Not allowed'
					}
				]
			: []),
		...describeRules(preset.ruleset)
	]);

	/** Makes an invite Room on the server, and opens its link to wait there for a friend. */
	const invite = async () => {
		if (creating) return;
		creating = true;
		failure = null;
		const room =
			(await startSession()) &&
			(await createRoom({ rules: rulesetCode(preset.ruleset), seat, takebacks, clock: null }));
		// Still creating while the page goes to the Room, so a second press makes no second Room.
		if (room) return goto(resolve('/r/[room]', { room }));
		creating = false;
		failure = 'Kokonoe’s server couldn’t make the Room. Invite a friend once you’re online.';
	};

	const start = () => {
		if (kind === 'friend') return invite();
		const random: Player = crypto.getRandomValues(new Uint8Array(1))[0] & 1 ? 'O' : 'X';
		const opponent: Opponent =
			kind === 'computer'
				? { kind, level, seat: seat === 'Random' ? random : seat }
				: { kind: 'hotseat' };
		goto(resolve(`/play${playQuery({ code: rulesetCode(preset.ruleset), opponent })}`));
	};

	const card =
		'flex cursor-pointer items-start gap-3 rounded-[10px] border-[1.5px] border-pencil bg-card/60 p-4 has-checked:border-2 has-checked:border-foreground has-checked:bg-card has-checked:shadow-[4px_4px_0_var(--muted)] has-disabled:cursor-not-allowed has-disabled:opacity-55';
	const legend = 'mb-3 p-0 font-mono text-[13px] tracking-[0.08em] text-muted-foreground';
</script>

<svelte:head>
	<title>New game · Kokonoe</title>
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

	<main class="mx-auto flex max-w-[1240px] flex-wrap items-start gap-8 px-8 pb-12">
		<div class="flex min-w-0 flex-[999_1_600px] flex-col gap-7">
			<h1 class="m-0 font-display text-5xl font-semibold tracking-[-0.02em]">New game</h1>

			<fieldset class="m-0 flex min-w-0 flex-col gap-3 border-0 p-0">
				<legend class={legend}>1 · OPPONENT</legend>
				<div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3">
					{#each [{ value: 'computer', name: 'Computer', detail: 'Offline, on this device' }, { value: 'hotseat', name: 'Pass and play', detail: 'Two players, one screen' }, { value: 'friend', name: 'A friend', detail: 'Online, by invite link' }] as const as option (option.value)}
						<label class={card}>
							<input
								type="radio"
								name="opponent"
								value={option.value}
								bind:group={kind}
								class="mt-0.5 size-5 accent-foreground"
							/>
							<span class="flex flex-col gap-0.5">
								<span class="font-semibold">{option.name}</span>
								<span class="text-sm text-muted-foreground">{option.detail}</span>
							</span>
						</label>
					{/each}
				</div>
				{#if kind !== 'hotseat'}
					<div class="flex flex-wrap items-center gap-x-6 gap-y-3">
						{#if kind === 'computer'}
							<label class="flex items-center gap-3 text-sm font-medium">
								Computer level
								<select
									bind:value={level}
									class="min-h-11 rounded-md border-[1.5px] border-foreground bg-card px-3 text-[15px]"
								>
									{#each LEVELS as n (n)}
										<option value={n}>Level {n} of 8</option>
									{/each}
								</select>
							</label>
						{/if}
						<fieldset class="m-0 flex items-center gap-2 border-0 p-0">
							<legend class="float-left mr-3 p-0 text-sm font-medium">You play</legend>
							{#each ['X', 'O', 'Random'] as const as option (option)}
								<label
									class="inline-flex min-h-11 cursor-pointer items-center gap-2 rounded-md border-[1.5px] border-pencil px-3 has-checked:border-foreground has-checked:bg-card"
								>
									<input
										type="radio"
										name="seat"
										value={option}
										checked={seat === option}
										onchange={() => (seats[seatOf] = option)}
										class="accent-foreground"
									/>
									{option}
								</label>
							{/each}
						</fieldset>
						{#if kind === 'friend'}
							<label class="flex min-h-11 cursor-pointer items-center gap-3 text-sm font-medium">
								<input type="checkbox" bind:checked={takebacks} class="size-5 accent-foreground" />
								Allow takebacks
							</label>
						{/if}
					</div>
				{/if}
			</fieldset>

			<fieldset class="m-0 flex min-w-0 flex-col gap-3 border-0 p-0">
				<legend class={legend}>2 · VARIANT</legend>
				<div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3">
					{#each SHELF as option (option.name)}
						<label class={card}>
							<input
								type="radio"
								name="variant"
								value={option.name}
								bind:group={chosen}
								class="mt-0.5 size-5 accent-foreground"
							/>
							<span class="flex flex-col gap-0.5">
								<span class="font-display text-[19px] font-semibold">{option.name}</span>
								<span class="text-sm text-muted-foreground">
									{option.blurb}
								</span>
							</span>
						</label>
					{/each}
				</div>
			</fieldset>
		</div>

		<aside
			aria-label="Your game"
			class="flex min-w-0 flex-[1_1_320px] flex-col gap-4 rounded-[10px] border-[1.5px] border-foreground bg-card p-6 shadow-[6px_6px_0_var(--muted)]"
		>
			<span class="font-mono text-[13px] tracking-[0.08em] text-muted-foreground">YOUR GAME</span>
			<h2 class="m-0 font-display text-3xl leading-[1.1] font-semibold">
				{preset.name}
				{KINDS[kind].title}
			</h2>
			<dl class="m-0 flex flex-col gap-2.5 text-[15px]">
				{#each summary as row (row.label)}
					<div>
						<dt class="text-sm text-muted-foreground">{row.label}</dt>
						<dd class="m-0 font-medium">{row.value}</dd>
					</div>
				{/each}
			</dl>
			<button
				type="button"
				class="min-h-13 rounded-md bg-primary text-[17px] font-semibold text-primary-foreground aria-disabled:opacity-60"
				aria-disabled={creating}
				onclick={start}
			>
				{creating ? 'Creating the Room…' : KINDS[kind].start}
			</button>
			{#if failure}
				<p class="m-0 text-sm" role="alert">{failure}</p>
			{/if}
		</aside>
	</main>
</div>
