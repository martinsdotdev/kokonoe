import { beforeAll, describe, expect, it } from 'vitest';
import {
	Game,
	loadEngine,
	newSeed,
	presets,
	type Cell,
	type Objective,
	type Opening,
	type PlayResult,
	type Ruleset
} from './engine.js';

/** Cells written as their digits, as in the engine's tests: `"42"` is Space 4, then Space 2. */
const cells = (text: string): Cell[] =>
	text
		.split(/\s+/)
		.filter(Boolean)
		.map((digits) => [...digits].reduce((index, digit) => index * 9 + Number(digit), 0));
const cell = (digits: string) => cells(digits)[0];

/** X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go. */
const TOP_ROW = '03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28';

const SEED = '00000000005eed00';

let STANDARD: Ruleset;

beforeAll(async () => {
	await loadEngine();
	STANDARD = presets()[0].ruleset;
});

/** A Standard game with `moves` played in order. */
const standard = (moves: string) => {
	const game = new Game(STANDARD, SEED);
	for (const move of cells(moves)) {
		expect(game.play(game.ply(), move)).toHaveProperty('Now');
	}
	return game;
};

const illegal = (game: Game, digits: string): PlayResult => game.play(game.ply(), cell(digits));

describe('a Standard game through the bindings', () => {
	it('starts anywhere, with X to move', () => {
		expect(standard('').status()).toEqual({
			ToMove: { player: 'X', sent: 'FirstMove', moves: [...Array(81).keys()] }
		});
	});

	it('plays to a line win', () => {
		const game = new Game(STANDARD, newSeed());
		for (const [i, move] of cells(TOP_ROW).entries()) {
			const result = game.play(i, move);
			if (i < 16) {
				expect(result).toEqual({ Now: game.status() });
				expect(game.status()).toMatchObject({ ToMove: { player: i % 2 ? 'X' : 'O' } });
			}
		}
		expect(game.status()).toEqual({ Over: { Objective: 'X' } });
		expect(game.boardStatus([])).toEqual({ Won: 'X' });
		expect(game.boardStatus([0])).toEqual({ Won: 'X' });
		expect(game.boardStatus([3])).toBe('Open');
		expect(game.wonCount('X')).toBe(3);
		expect(game.wonCount('O')).toBe(0);
		expect([game.mark(cell('03')), game.mark(cell('30')), game.mark(cell('88'))]).toEqual([
			'X',
			'O',
			undefined
		]);
		expect(game.moves()).toEqual(cells(TOP_ROW));
		expect(illegal(game, '88')).toEqual({ Illegal: 'GameOver' });
	});

	it('sends the opponent to the Local board of the Space, or frees the move', () => {
		expect(standard('42').status()).toMatchObject({ ToMove: { player: 'O', sent: { To: [2] } } });

		// X wins Local board 0 with 0·5, then O sends X back to it.
		const free = standard('03 30 04 40 05 50').status();
		expect(free).toMatchObject({ ToMove: { sent: { Free: { target: [0], within: [] } } } });
		expect('ToMove' in free && free.ToMove.moves.length).toBe(72 - 3);
	});

	it('returns each refusal as a value naming the Board to flash', () => {
		const game = standard('44');
		expect(illegal(game, '44')).toEqual({ Illegal: 'Occupied' });
		expect(illegal(game, '50')).toEqual({ Illegal: { OutsideActiveBoards: { within: [4] } } });
		expect(illegal(standard('03 30 04 40 05'), '00')).toEqual({
			Illegal: { BeneathDecided: { board: [0] } }
		});
		expect(game.moves()).toEqual([cell('44')]);
	});

	it('makes resends harmless and refuses plies out of order', () => {
		const game = standard('42 24');
		expect(game.play(1, cell('24'))).toBe('Already');
		expect(game.play(1, cell('25'))).toEqual({ OutOfOrder: { want: 2, got: 1 } });
		expect(game.play(3, cell('40'))).toEqual({ OutOfOrder: { want: 2, got: 3 } });
		expect(game.ply()).toBe(2);
	});

	it('undoes back to the first move', () => {
		const game = standard('42 24 40');
		expect([game.undo(), game.undo(), game.undo(), game.undo()]).toEqual([
			cell('40'),
			cell('24'),
			cell('42'),
			undefined
		]);
		expect(game.status()).toEqual(standard('').status());
	});

	it('opens its own Record, and reports the first move the rules reject', () => {
		const game = standard(TOP_ROW);
		const record = game.record();
		expect(record).toEqual({
			rules_version: 1,
			ruleset: STANDARD,
			seed: SEED,
			moves: cells(TOP_ROW)
		});
		const opened = Game.open(record);
		expect(opened).toBeInstanceOf(Game);
		expect(opened instanceof Game && opened.status()).toEqual(game.status());

		const twice = { ...standard('42 24').record(), moves: cells('42 24 24') };
		expect(Game.open(twice)).toEqual({ index: 2, error: 'Occupied' });
	});

	it('deals a Random opening from the Seed, outside the Record', () => {
		const rules: Ruleset = { ...STANDARD, opening: { Random: 4 } };
		const game = new Game(rules, SEED);
		expect(game.opening()).toHaveLength(4);
		expect(game.ply()).toBe(4);
		expect(game.record().moves).toEqual([]);
		expect(new Game(rules, SEED).opening()).toEqual(game.opening());

		const opened = Game.open(game.record());
		expect(opened instanceof Game && opened.opening()).toEqual(game.opening());
	});
});

describe('values crossing the boundary', () => {
	it('round-trips all 1,440 Rulesets', () => {
		const objectives: Objective[] = [
			'Line',
			...[...Array(9).keys()].map((n) => ({ FirstTo: n + 1 }))
		];
		const openings: Opening[] = ['Free', ...[...Array(8).keys()].map((k) => ({ Random: k + 1 }))];
		let count = 0;
		for (const depth of [2, 3] as const)
			for (const continuation of ['Off', 'On'] as const)
				for (const drawn_owner of ['Neither', 'Both'] as const)
					for (const objective of objectives)
						for (const fallback of ['Draw', 'MostWon'] as const)
							for (const opening of openings) {
								const rules = { depth, continuation, drawn_owner, objective, fallback, opening };
								expect(new Game(rules, SEED).ruleset()).toEqual(rules);
								count++;
							}
		expect(count).toBe(1440);
	});

	it('writes the Presets in the JSON of spec §4.2', () => {
		expect(presets().map((preset) => preset.name)).toEqual([
			'Standard',
			'Forced continuation',
			'Tic-Tac-Ku',
			'Count tiebreak',
			'Deep'
		]);
		expect(JSON.stringify(STANDARD)).toBe(
			'{"depth":2,"continuation":"Off","drawn_owner":"Neither","objective":"Line","fallback":"Draw","opening":"Free"}'
		);
		expect(presets()[2].ruleset.objective).toEqual({ FirstTo: 5 });
	});

	it('throws on data of the wrong shape, with the engine’s messages', () => {
		const game = standard('');
		const record = game.record();
		const malformed: [() => unknown, RegExp][] = [
			[() => new Game({ ...STANDARD, depth: 4 as 2 }, SEED), /depth 4 is not 2 or 3/],
			[() => new Game({ ...STANDARD, objective: { FirstTo: 10 } }, SEED), /FirstTo\(10\)/],
			[() => new Game({ ...STANDARD, extra: 1 } as Ruleset, SEED), /a Ruleset is/],
			[() => new Game(STANDARD, '5eed'), /a Seed is 16 hex digits/],
			[() => game.play(0, 81), /cell index 81/],
			[() => game.play(0, 1.5), /a Cell is its index/],
			[() => game.boardStatus([4, 2]), /names no Board/],
			[() => game.wonCount('Z' as 'X'), /a Player is/],
			[() => Game.open({ ...record, rules_version: 2 }), /rules version 2/],
			[() => Game.open({ ...record, moves: [-1] }), /a Cell is its index/]
		];
		for (const [call, message] of malformed) expect(call).toThrow(message);
	});
});

it('loads the same module inside a Worker', async () => {
	const worker = new Worker(new URL('./engine-test.worker.ts', import.meta.url), {
		type: 'module'
	});
	const reply = new Promise((resolve, reject) => {
		worker.addEventListener('message', (event) => resolve(event.data));
		worker.addEventListener('error', reject);
	});
	const game = standard('42 24');
	worker.postMessage(game.record());
	expect(await reply).toEqual(game.status());
	worker.terminate();
});
