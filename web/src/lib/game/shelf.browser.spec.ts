import { beforeAll, expect, it } from 'vitest';
import { loadEngine, presets } from '#lib/engine.js';
import { presetByCode, SHELF } from './shelf.js';

beforeAll(() => loadEngine());

it("holds the engine's Presets, in its order", () => {
	expect(SHELF.map(({ name, ruleset }) => ({ name, ruleset }))).toEqual(presets());
});

it('finds each Preset by its code, and only Deep waits', () => {
	expect(presetByCode('cont+first5+most')?.name).toBe('Tic-Tac-Ku');
	expect(presetByCode('first5')).toBeUndefined();
	expect(SHELF.filter((preset) => !preset.playable).map((preset) => preset.name)).toEqual(['Deep']);
});
