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
 * Checks a game file with the engine, and with the tags it has: every move legal, the plies it
 * writes as dealt the ones its Seed deals, and its Result how the moves end. Returns how it
 * stopped, a resignation or an agreed draw, which only a `.utt` file's tags can say. Throws a
 * FileProblem.
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
		if (termination === 'resign' && 'Over' in status) {
			throw new FileProblem('It says a player resigned after the game ended.');
		}
		if (termination === 'agreed draw' && 'Over' in status) {
			throw new FileProblem('It says the players agreed a draw after the game ended.');
		}
		const stopped = stoppedFrom(said, termination);
		if (termination === 'resign' && !stopped) {
			throw new FileProblem('It says a player resigned, but its Result names no winner.');
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
