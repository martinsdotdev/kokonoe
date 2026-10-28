<!--
	The board on a phone, and for Deep on every screen (spec §7.3, "Step into boards"): one Board
	at a time. The Global view shows the whole board, where a tap on a Board steps into it, one
	level a tap: at depth 3 into a middle Board, then one of its Local boards. Inside a Local
	board, drawn large, the Cells take the moves. A breadcrumb steps back out, a level a crumb.

	The view follows the game: after each move it shows the Board the player is Sent to, the
	Board a Free move may go anywhere inside, or the Global view on the first move and once the
	game is over. The computer's move first lands in view, then the view follows. Taps wait until
	a step has settled, so a double tap can't play into the Board that just came in. In a replay,
	`landed`, the view shows each move where it landed instead.
-->
<script lang="ts">
	import type { Board, Cell } from '#lib/engine.js';
	import { prefersReducedMotion } from 'svelte/motion';
	import GlobalBoard from './Board.svelte';
	import BoardTile from './BoardTile.svelte';
	import MiniBoard from './MiniBoard.svelte';
	import { boardName } from './notation.js';
	import { key, lineage, localOf, same, type BoardView } from './view.js';

	interface Props {
		view: BoardView;
		onplay?: (cell: Cell) => void;
		/** A Board to flash, after an illegal tap that names it. A new `tick` flashes again. */
		flash?: { board: Board; tick: number } | null;
		/** The last tap was refused: the view steps to the Board the refusal names. */
		refusal?: { board: Board | null; tick: number } | null;
		/** The game is over: Cells take no clicks. */
		disabled?: boolean;
		/** `view` is the computer's move, to show landing before following. */
		replied?: boolean;
		/** Shows the last move where it landed, never following: a replay. */
		landed?: boolean;
	}

	let {
		view,
		onplay,
		flash = null,
		refusal = null,
		disabled = false,
		replied = false,
		landed = false
	}: Props = $props();

	/** How long a step takes, as the prototype had it. */
	const STEP_MS = 220;
	/** How long the computer's move stays in view before the view follows. */
	const HOLD_MS = 600;

	/** Where the player to move plays: the Sent target, the Board a Free move is inside, or anywhere. */
	const follow = (view: BoardView): Board => view.within ?? view.sent ?? [];

	/**
	 * Holding the computer's move in view. It reads `view` so each new view starts a new hold,
	 * and the hold's timer or a step ends it by assignment.
	 */
	let holding = $derived.by(() => {
		void view;
		return replied;
	});

	/**
	 * What's shown, by its path, `[]` being the Global board: the Board a refusal names, the
	 * last move where it landed, or where to play. A step assigns it until the game next changes.
	 */
	let focus: Board = $derived.by(() => {
		if (refusal?.board) return refusal.board;
		if (holding || landed) return view.last === null ? [] : localOf(view.last, view.depth);
		return follow(view);
	});

	$effect(() => {
		if (!holding) return;
		const timer = setTimeout(() => (holding = false), HOLD_MS);
		return () => clearTimeout(timer);
	});

	/** No step is animating, so taps go through. Tests wait on it as `data-settled`. */
	let settled = $state(true);

	const step = (to: Board) => {
		if (!settled) return;
		holding = false;
		focus = to;
	};

	let root: HTMLElement;
	let stage: HTMLElement;
	// Plain variables, not state: the effects below read and write them, and nothing renders them.
	/** The focus last drawn: the step's animation goes from it. */
	let drawn: Board | null = null;
	/** Keyboard focus was in the board as the view changed, read before the DOM changes. */
	let focused = false;

	$effect.pre(() => {
		void focus;
		focused = root?.contains(document.activeElement) ?? false;
	});

	$effect(() => {
		const to = focus;
		const from = drawn;
		drawn = to;
		// Every move builds new paths, so compare them by value.
		if (from === null || same(from, to)) return;
		const into = same(from, to.slice(0, from.length));
		const out = same(to, from.slice(0, to.length));
		// Stepping in or out, the focused element is gone, so pass focus on: to the Board entered,
		// or to the button of the Board just left. From Board to Board the Cells stay.
		if (focused && !root.contains(document.activeElement)) {
			const next = out
				? stage.querySelector<HTMLElement>(`[data-board="${key(from.slice(0, to.length + 1))}"]`)
				: root.querySelector<HTMLElement>('[aria-current]');
			next?.focus();
		}
		// Grow from where the Board stepped into, or out of, sits in its parent.
		const space = out ? from[to.length] : into ? to[from.length] : to[to.length - 1];
		if (prefersReducedMotion.current) return;
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

<div
	class="stepped"
	bind:this={root}
	data-focus={focus.length ? key(focus) : 'global'}
	data-settled={settled || undefined}
>
	<nav aria-label="Where you are">
		<!-- Safari drops a list's semantics with its bullets, unless it's given the role back. -->
		<ol role="list">
			{#each lineage(focus).slice(0, -1) as crumb (key(crumb))}
				<li>
					<button type="button" class="crumb out" onclick={() => step(crumb)}>
						<span class="mini"><MiniBoard {view} board={crumb} /></span>
						{crumb.length ? boardName(crumb) : 'Global'}
					</button>
				</li>
			{/each}
			<li>
				{#if focus.length}
					<h2 class="crumb" aria-current="location" tabindex="-1">{boardName(focus)}</h2>
				{:else}
					<span class="crumb" aria-current="location">Global</span>
				{/if}
			</li>
		</ol>
	</nav>
	<div bind:this={stage}>
		{#if focus.length}
			<BoardTile
				{view}
				board={focus}
				{flash}
				{disabled}
				large
				onstep={step}
				onplay={(cell) => settled && onplay?.(cell)}
			/>
		{:else}
			<GlobalBoard {view} {flash} {disabled} onstep={step} />
		{/if}
	</div>
</div>

<style>
	.stepped {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
		/* On a desktop the whole board fits the window under the header, but never shrinks past
		   playing size: a short window, or a zoomed one, scrolls instead. */
		max-width: min(600px, max(20rem, 100svh - 12rem));
	}

	ol {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 2px;
		margin: 0;
		padding: 0;
		list-style: none;
		/* Three crumbs fit a 390 px phone on one line. */
		font: 500 13px/1 var(--font-mono);
		letter-spacing: 0.02em;
		text-transform: uppercase;
	}
	li {
		display: flex;
		align-items: center;
	}
	li + li::before {
		content: '›';
		content: '›' / '';
		padding-inline: 3px;
	}

	.crumb {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 44px;
		margin: 0;
		font: inherit;
		white-space: nowrap;
	}
	/* The heading takes focus only when the view steps in, so it shows a ring only then. */
	h2.crumb:focus {
		outline: none;
	}
	h2.crumb:focus-visible {
		outline: 3px solid var(--ring);
		outline-offset: 2px;
	}
	.crumb.out {
		padding: 0;
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
		width: 28px;
	}
</style>
