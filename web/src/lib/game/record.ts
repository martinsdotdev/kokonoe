/** Opening a saved Record or a game file with the engine, or saying in words why it can't be. */
import { Game, type Player, type Record, type ReplayError } from '#lib/engine.js';
import { FileProblem, result, type ParsedFile } from './gamefile.js';
import { playerAt } from './moves.js';
import { notation } from './notation.js';
import { resultText, type End } from './result.js';

/** A saved game this version of Kokonoe can't play on or replay: its message says why. */
export class UnplayableGame extends Error {}

export const illegalMove = (index: number) =>
	new UnplayableGame(`Its move ${index + 1} isn’t legal.`);

/**
 * The Game `record` leads to. Throws an UnplayableGame when this version of Kokonoe can't read
 * the Record, or one of its moves isn't legal.
 */
export const openRecord = (record: Record): Game => {
	// Only `open` checks the Record's rules version.
	let opened: Game | ReplayError;
	try {
		opened = Game.open(record);
	} catch {
		throw new UnplayableGame('It was saved in a form this version of Kokonoe can’t read.');
	}
	if (!(opened instanceof Game)) throw illegalMove(opened.index);
	return opened;
};

/**
 * Checks a game file with the engine, as far as its tags let it: every move legal, the plies it
 * writes as dealt the ones its Seed deals, and its Result how the moves end. Returns who
 * resigned, which only a `.utt` file's tags can say. Throws a FileProblem.
 */
export const openFile = (parsed: ParsedFile): { resigned: Player | null } => {
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
		let resigned: Player | null = null;
		if (tags.get('Termination') === 'resign') {
			if ('Over' in status)
				throw new FileProblem('It says a player resigned after the game ended.');
			if (said !== '1-0' && said !== '0-1') {
				throw new FileProblem('It says a player resigned, but its Result names no winner.');
			}
			resigned = said === '1-0' ? 'O' : 'X';
		}
		const end: End | null = resigned
			? { resigned }
			: 'Over' in status
				? { outcome: status.Over }
				: null;
		if (said !== undefined && said !== result(end)) {
			const won = { X: game.wonCount('X'), O: game.wonCount('O') };
			const truth = end ? resultText(end, record.ruleset, won) : 'the game isn’t over';
			throw new FileProblem(`It says ${said}, but ${truth}.`);
		}
		return { resigned };
	} finally {
		game.free();
	}
};
