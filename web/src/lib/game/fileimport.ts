/**
 * Opening a game file with the engine: every move legal, the dealt plies the Seed's, and the
 * Result how the moves end, before the Replay viewer shows it.
 */
import { Game, type ReplayError } from '#lib/engine.js';
import { FileProblem, type ParsedFile } from './fileparse.js';
import { playerAt } from './moves.js';
import { notation } from './notation.js';
import { endOf, result, resultText, stoppedFrom, type Stopped } from './result.js';

/**
 * What each Termination that stops a game says happened, for a refusal to quote. A Map, so a tag
 * such as `constructor` finds nothing.
 */
const STOPS = new Map([
	['resign', 'a player resigned'],
	['agreed draw', 'the players agreed a draw'],
	['timeout', 'a player ran out of time'],
	['claim', 'a player claimed the game'],
	['abandoned', 'both players left']
]);

/**
 * Checks a game file with the engine, and with the tags it has: every move legal, the plies it
 * writes as dealt the ones its Seed deals, and its Result how the moves end. Returns how it
 * stopped, a resignation, an agreed draw, a loss on time, a Claim or an abandonment, which only a
 * `.utt` file's tags can say. Throws a FileProblem.
 */
export const openFile = (parsed: ParsedFile): { stopped: Stopped | null } => {
	const { record, tags } = parsed;
	let opened: Game | ReplayError;
	try {
		opened = Game.open(record);
	} catch (error) {
		throw new FileProblem(`This file can’t be read: ${(error as Error).message}.`);
	}
	if (!(opened instanceof Game)) {
		// Number the move as the move list does: by its ply, the dealt opening counted.
		const deal = new Game(record.ruleset, record.seed);
		const ply = deal.opening().length + opened.index + 1;
		deal.free();
		const cell = notation(record.moves[opened.index], record.ruleset.depth);
		throw new FileProblem(`Move ${ply}, ${playerAt(ply)} ${cell}, isn’t legal.`);
	}
	const game = opened;
	try {
		const opening = game.opening();
		if (parsed.dealt.length && parsed.dealt.join() !== opening.join()) {
			throw new FileProblem(`Its dealt opening isn’t what Seed ${record.seed} deals.`);
		}
		const status = game.status();
		const said = tags.get('Result');
		const termination = tags.get('Termination');
		const stop = termination === undefined ? undefined : STOPS.get(termination);
		if (stop && 'Over' in status) {
			throw new FileProblem(`It says ${stop} after the game ended.`);
		}
		const stopped = stoppedFrom(said, termination);
		if (stop && !stopped) {
			const names = termination === 'claim' ? 'neither a winner nor a draw' : 'no winner';
			throw new FileProblem(`It says ${stop}, but its Result names ${names}.`);
		}
		const end = endOf(stopped, status);
		if (said !== undefined && said !== result(end)) {
			const won = { X: game.wonCount('X'), O: game.wonCount('O') };
			const truth = end ? resultText(end, record.ruleset, won) : 'the game isn’t over';
			throw new FileProblem(`It says ${said}, but ${truth}.`);
		}
		return { stopped };
	} finally {
		game.free();
	}
};
