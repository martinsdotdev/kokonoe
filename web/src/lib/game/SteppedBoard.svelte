<!--
	The board on a phone (spec §7.3, "Step into boards"): one Board at a time. The Global view
	shows the whole board, where a tap on a Local board steps into it; inside, that Local board is
	drawn large and its Cells take the moves. A breadcrumb steps back out.

	The view follows the game: after each move it shows the Board the player is Sent to, or the
	Global view when the move is Free, the first, or the game is over. The computer's move first
	lands in view, then the view follows. Taps wait until a step has settled, so a double tap
	can't play into the Board that just came in.
-->
<script lang="ts">
	import type { Cell } from '#lib/engine.js';
	import { prefersReducedMotion } from 'svelte/motion';
	import Board from './Board.svelte';
	import LocalBoard from './LocalBoard.svelte';
	import MiniBoard from './MiniBoard.svelte';
	import { boardLabel } from './notation.js';
	import type { BoardView } from './view.js';

	interface Props {
		view: BoardView;
		onplay?: (cell: Cell) => void;
		/** A Local board to flash, after an illegal tap that names it. A new `tick` flashes again. */
		flash?: { space: number; tick: number } | null;
		/** The game is over: Cells take no clicks. */
		disabled?: boolean;
		/** `view` is the computer's move, to show landing before following. */
		replied?: boolean;
	}

	let { view, onplay, flash = null, disabled = false, replied = false }: Props = $props();

	/** The Global board, or a Local board by its Space. */
	type Focus = 'global' | number;

	/** How long a step takes, as the prototype had it. */
	const STEP_MS = 220;
	/** How long the computer's move stays in view before the view follows. */
	const HOLD_MS = 600;

	/** Where the player to move plays: the Sent target, or anywhere. */
	const follow = (view: BoardView): Focus =>
		view.sent === null || view.free ? 'global' : view.sent;

	/** Holding the computer's move in view. A new view starts a new hold, and a step ends it. */
	let holding = $derived.by(() => {
		void view;
		return replied;
	});

	/** What's shown: a flashed Board, the computer's move, or where to play. Steps assign it. */
	let focus: Focus = $derived.by(() => {
		if (flash) return flash.space;
		if (holding && view.last !== null) return Math.floor(view.last / 9);
		return follow(view);
	});

	$effect(() => {
		if (!holding) return;
		const timer = setTimeout(() => (holding = false), HOLD_MS);
		return () => clearTimeout(timer);
	});

	/** No step is animating, so taps go through. */
	let settled = $state(true);

	const step = (to: Focus) => {
		if (!settled) return;
		holding = false;
		focus = to;
	};

	let root: HTMLElement;
	let stage: HTMLElement;
	/** The focus last drawn, not reactive: the step's animation goes from it. */
	let drawn: Focus | null = null;
	/** Keyboard focus was in the board as the view changed. */
	let focused = false;

	$effect.pre(() => {
		void focus;
		focused = root?.contains(document.activeElement) ?? false;
	});

	$effect(() => {
		const to = focus;
		const from = drawn;
		drawn = to;
		if (from === null || from === to) return;
		// Between the Global view and a Board the focused element is gone, so pass focus on: to the
		// Board just entered, or to the button of the Board just left. Board to Board keeps its Cells.
		if (focused && !root.contains(document.activeElement)) {
			const next = to === 'global' ? `[data-space="${from}"]` : '[aria-current]';
			root.querySelector<HTMLElement>(next)?.focus();
		}
		// Grow from the place on the Global board of the Board entered, or the Board left.
		const space = to === 'global' ? from : to;
		if (prefersReducedMotion.current || space === 'global') return;
		stage.style.transformOrigin = `${(space % 3) * 50}% ${Math.floor(space / 3) * 50}%`;
		settled = false;
		const animation = stage.animate(
			[
				{ transform: 'scale(0.82)', opacity: 0.35 },
				{ transform: 'none', opacity: 1 }
			],
			{ duration: STEP_MS, easing: 'cubic-bezier(0.2, 0, 0, 1)' }
		);
		animation.finished.then(
			() => (settled = true),
			() => {}
		);
		return () => {
			animation.cancel();
			settled = true;
		};
	});
</script>

<div class="stepped" bind:this={root} data-focus={focus} data-settled={settled || undefined}>
	<nav aria-label="Where you are">
		<ol>
			{#if focus === 'global'}
				<li><span class="crumb" aria-current="location">Global</span></li>
			{:else}
				<li>
					<button type="button" class="crumb out" onclick={() => step('global')}>
						<span class="mini" aria-hidden="true"><MiniBoard {view} label="" /></span>
						Global
					</button>
				</li>
				<li>
					<span class="crumb" aria-current="location" tabindex="-1">
						Board {boardLabel([focus])}
					</span>
				</li>
			{/if}
		</ol>
	</nav>
	<div class="stage" bind:this={stage}>
		{#if focus === 'global'}
			<Board {view} {flash} {disabled} onstep={step} />
		{:else}
			<LocalBoard
				{view}
				local={view.locals[focus]}
				{flash}
				{disabled}
				large
				onplay={(cell) => settled && onplay?.(cell)}
			/>
		{/if}
	</div>
</div>

<style>
	.stepped {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
		max-width: 600px;
	}

	ol {
		display: flex;
		align-items: center;
		gap: 4px;
		margin: 0;
		padding: 0;
		list-style: none;
		font: 500 14px/1 var(--font-mono);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}
	li {
		display: flex;
		align-items: center;
	}
	li + li::before {
		content: '›' / '';
		padding-inline: 4px;
	}

	.crumb {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		min-height: 44px;
		outline: none;
	}
	.crumb.out {
		padding: 0 4px 0 0;
		border: 0;
		background: none;
		color: inherit;
		font: inherit;
		letter-spacing: inherit;
		text-transform: inherit;
		text-decoration: underline;
		text-underline-offset: 3px;
		cursor: pointer;
	}
	.crumb.out:focus-visible {
		outline: 3px solid var(--ring);
		outline-offset: 2px;
	}
	.mini {
		display: block;
		width: 40px;
	}

	.stage {
		display: flex;
		justify-content: center;
	}
</style>
