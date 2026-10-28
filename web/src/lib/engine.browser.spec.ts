import { beforeAll, describe, expect, it } from 'vitest';
import {
	Game,
	loadEngine,
	newSeed,
	presets,
	think,
	type Cell,
	type Continuation,
	type DrawnOwner,
	type Fallback,
	type Objective,
	type Opening,
	type PlayResult,
	type Player,
	type Ruleset
} from './engine.js';

/** Cells written as their digits, as in the engine's tests: `"42"` is Space 4, then Space 2. */
const cells = (text: string): Cell[] =>
	text
		.split(/\s+/)
		.filter(Boolean)
		.map((digits) => [...digits].reduce((index, digit) => index * 9 + Number(digit), 0));
const cell = (digits: string) => cells(digits)[0];

const range = (n: number) => [...Array(n).keys()];

/** A variant's name: the string itself, or the one key of `{ Variant: data }`. */
type Tag<T> = T extends string ? T : keyof T;

/**
 * Holds a list to every member of a union. Leaving one out is a type error, so when the engine
 * gains a variant, `pnpm check` fails here until these tests cover it.
 */
const every =
	<T extends string>() =>
	<const L extends readonly T[]>(
		list: L & ([Exclude<T, L[number]>] extends [never] ? unknown : never)
	) =>
		list;

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
			ToMove: { player: 'X', sent: 'FirstMove', moves: range(81) }
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
		const players = every<Player>()(['X', 'O']);
		expect(players.map((player) => game.wonCount(player))).toEqual([3, 0]);
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
		expect(game.play(70_000, cell('40')), 'past the engine’s plies').toEqual({
			OutOfOrder: { want: 2, got: 70_000 }
		});
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
		const opening = game.opening();
		// The engine's pinned vector for Seed 0x5eed00 (crates/engine/tests/openings.rs), so the
		// hex Seed reads as the same u64 on both sides.
		expect(opening).toEqual(cells('10 08 82 20'));
		expect(game.ply()).toBe(4);
		expect(game.record().moves).toEqual([]);
		expect(new Game(rules, SEED).opening()).toEqual(opening);
		expect(game.play(0, opening[0]), 'a dealt ply resent').toBe('Already');

		const opened = Game.open(game.record());
		expect(opened instanceof Game && opened.opening()).toEqual(opening);
	});

	it('reads the State at any ply, the dealt opening included', () => {
		const game = new Game({ ...STANDARD, opening: { Random: 4 } }, SEED);
		game.play(4, cell('03'));
		const marks = (ply: number) => {
			const state = game.stateAt(ply)!;
			try {
				return [...game.opening(), cell('03')].map((at) => state.mark(at) ?? null);
			} finally {
				state.free();
			}
		};
		expect(marks(0)).toEqual([null, null, null, null, null]);
		expect(marks(2)).toEqual(['X', 'O', null, null, null]);
		expect(marks(5)).toEqual(['X', 'O', 'X', 'O', 'X']);

		const end = game.stateAt(5)!;
		expect(end.ply()).toBe(5);
		expect(end.status()).toEqual(game.status());
		expect(end.ruleset()).toEqual(game.ruleset());
		expect(end.boardStatus([1])).toEqual(game.boardStatus([1]));
		expect(end.wonCount('X')).toBe(game.wonCount('X'));
		end.free();

		expect(game.stateAt(6), 'past the last ply').toBeUndefined();
		expect(() => game.stateAt(1.5)).toThrow('a Ply is a whole number');
		expect(() => game.stateAt(0)!.mark(81)).toThrow();
	});
});

describe('values crossing the boundary', () => {
	it('round-trips all 1,440 Rulesets', () => {
		const objectives = every<Tag<Objective>>()(['Line', 'FirstTo']).flatMap((tag): Objective[] =>
			tag === 'Line' ? [tag] : range(9).map((n) => ({ FirstTo: n + 1 }))
		);
		const openings = every<Tag<Opening>>()(['Free', 'Random']).flatMap((tag): Opening[] =>
			tag === 'Free' ? [tag] : range(8).map((k) => ({ Random: k + 1 }))
		);
		let count = 0;
		for (const depth of [2, 3] as const)
			for (const continuation of every<Continuation>()(['Off', 'On']))
				for (const drawn_owner of every<DrawnOwner>()(['Neither', 'Both']))
					for (const objective of objectives)
						for (const fallback of every<Fallback>()(['Draw', 'MostWon']))
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
			[() => new Game(STANDARD, '5eed'), /a Seed is 16 lowercase hex digits/],
			[() => new Game(STANDARD, SEED.toUpperCase()), /a Seed is 16 lowercase hex digits/],
			[() => game.play(0, 81), /cell index 81/],
			[() => game.play(0, 1.5), /a Cell is its index/],
			[() => game.boardStatus([4, 2]), /names no Board/],
			[() => game.wonCount('Z' as 'X'), /a Player is/],
			[() => Game.open({ ...record, rules_version: 2 }), /rules version 2/],
			[() => Game.open({ ...record, moves: [-1] }), /a Cell is its index/]
		];
		for (const [call, message] of malformed) expect(call).toThrow(message);
	});

	it('reads only own data properties', () => {
		const inherited = Object.assign(Object.create(STANDARD), {
			a: 1,
			b: 2,
			c: 3,
			d: 4,
			e: 5,
			f: 6
		});
		const getter = Object.defineProperty({ ...STANDARD }, 'depth', {
			get: () => 2,
			enumerable: true
		});
		const { rules_version, ruleset, seed } = standard('').record();
		const inheritedMoves = Object.assign(Object.create({ moves: [] }), {
			...{ rules_version, ruleset, seed, extra: 1 }
		});
		expect(() => new Game(inherited, SEED)).toThrow(/a Ruleset is/);
		expect(() => new Game(getter, SEED)).toThrow(/a Ruleset is/);
		expect(() => Game.open(inheritedMoves)).toThrow(/a Record is/);
	});

	it('stays usable after a value that throws when read', () => {
		const game = standard('42');
		const { proxy: revoked, revoke } = Proxy.revocable([4], {});
		revoke();
		const trapped = new Proxy(
			{ ...STANDARD },
			{
				ownKeys: () => {
					throw new Error('trap');
				}
			}
		);
		expect(() => game.boardStatus(revoked)).toThrow(/a Board is/);
		expect(() => new Game(trapped, SEED)).toThrow(/a Ruleset is/);
		// A JS exception thrown through the WASM frames would have left the Game borrowed.
		expect(game.play(1, cell('24'))).toHaveProperty('Now');
		expect(game.undo()).toBe(cell('24'));
	});
});

describe('the computer', () => {
	it('answers with a legal move, the same one for the same seed', () => {
		const game = standard('42 24');
		const status = game.status();
		const moves = 'ToMove' in status ? status.ToMove.moves : [];
		for (const level of range(8).map((i) => i + 1)) {
			const move = think(game.record(), level, SEED);
			expect(moves).toContain(move);
			expect(think(game.record(), level, SEED)).toBe(move);
		}
	});

	it('takes an immediate win from Level 3', () => {
		const game = standard(TOP_ROW.split(' ').slice(0, -1).join(' '));
		expect(think(game.record(), 3, SEED)).toBe(cell('28'));
	});

	it('throws on a Level out of range or a finished game', () => {
		const record = standard('42').record();
		expect(() => think(record, 0, SEED)).toThrow(/Level 0 is outside 1 to 8/);
		expect(() => think(record, 9, SEED)).toThrow(/Level 9 is outside 1 to 8/);
		expect(() => think(standard(TOP_ROW).record(), 3, SEED)).toThrow(/the game is over/);
		expect(() => think({ ...record, moves: [0, 0] }, 3, SEED)).toThrow(/move 1 is illegal/);
	});
});

it('loads the same module inside a Worker', async () => {
	const worker = new Worker(new URL('./engine-test.worker.ts', import.meta.url), {
		type: 'module'
	});
	try {
		const reply = new Promise((resolve, reject) => {
			worker.addEventListener('message', (event) => resolve(event.data));
			worker.addEventListener('error', reject);
		});
		const game = standard('42 24');
		worker.postMessage(game.record());
		expect(await reply).toEqual(game.status());
	} finally {
		worker.terminate();
	}
});
