<!--
	Where the player to move may play, and why: anywhere on the first move, the Sent target, or,
	under the FREE MOVE stamp, anywhere open in the Board above a decided target, which at depth 2
	is the Global board. Its first line is the game's one live announcement, so it names the
	player, and says when a tap wasn't playable.
-->
<script lang="ts">
	import type { Board, Player, Sent } from '#lib/engine.js';
	import { boardLabel } from './notation.js';
	import { opponent } from './result.js';

	interface Props {
		player: Player;
		sent: Sent;
		/** The last move in Notation, which did the sending. */
		last: string | null;
		/** A tap was refused, and nothing has been played since. */
		refused?: boolean;
		/** At depth 3, a decided middle Board above the Sent target, which turned the move away. */
		decidedAbove?: Board | null;
	}

	let { player, sent, last, refused = false, decidedAbove = null }: Props = $props();

	const turn = $derived.by(() => {
		const by = `${opponent(player)}’s move ${last ?? ''}`.trim();
		if (sent === 'FirstMove') {
			return {
				free: false,
				line: `${player}: play anywhere.`,
				why: 'The first move may go in any Cell.'
			};
		}
		if ('To' in sent) {
			const board = boardLabel(sent.To);
			return {
				free: false,
				line: `${player}: play in board ${board}.`,
				why: `${by} Sent ${player} there.`
			};
		}
		const { target, within } = sent.Free;
		const to = `${by} Sent ${player} to board ${boardLabel(target)}`;
		if (within.length) {
			const board = boardLabel(within);
			return {
				free: true,
				line: `${player}: play anywhere in board ${board}.`,
				why: `${to}, which is decided, so every open board in board ${board} is ${player}’s.`
			};
		}
		const inside = decidedAbove ? `, inside board ${boardLabel(decidedAbove)},` : ',';
		return {
			free: true,
			line: `${player}: play in any open board.`,
			why: `${to}${inside} which is decided, so every open board is ${player}’s.`
		};
	});
	const line = $derived(refused ? `Not playable. ${turn.line}` : turn.line);
</script>

<section
	aria-label="Turn"
	class="flex flex-col gap-2 rounded-[10px] border-[1.5px] border-foreground bg-card p-4"
>
	<span class="font-mono text-xs tracking-[0.08em] text-muted-foreground">THIS TURN · {player}</span
	>
	{#if turn.free}
		<span
			class="-rotate-5 self-start border-2 px-3 py-1 font-mono text-sm font-medium tracking-[0.14em]"
			style:border-color="var(--o-text)"
			style:color="var(--o-text)"
			data-stamp
		>
			FREE MOVE
		</span>
	{/if}
	<p class="m-0 font-semibold" aria-live="polite">{line}</p>
	<p class="m-0 text-sm text-muted-foreground">{turn.why}</p>
</section>
