import { describe, expect, it, vi } from 'vitest';
import { updateNow } from './update.js';

/** A service worker that moves through the states it's given, one per tick. */
function worker(...states: ServiceWorkerState[]): ServiceWorker {
	const events = new EventTarget();
	const fake = Object.assign(events, { state: states[0] });
	(async () => {
		for (const state of states.slice(1)) {
			await Promise.resolve();
			fake.state = state;
			events.dispatchEvent(new Event('statechange'));
		}
	})();
	return fake as unknown as ServiceWorker;
}

/** A container whose registration holds only what `updateNow` reads. */
function container(registration?: object): ServiceWorkerContainer {
	return {
		getRegistration: async () => registration as ServiceWorkerRegistration | undefined
	} as ServiceWorkerContainer;
}

describe('updating now', () => {
	it('only reloads with no service worker', async () => {
		const reload = vi.fn();
		await updateNow({ container: undefined, reload });
		await updateNow({ container: container(), reload });
		expect(reload).toHaveBeenCalledTimes(2);
	});

	it('reloads once a newer build has activated', async () => {
		const newer = worker('installing', 'installed', 'activating', 'activated');
		const update = vi.fn(async () => {});
		const states: string[] = [];
		const reload = vi.fn(() => states.push(newer.state));
		await updateNow({ container: container({ update, installing: newer, waiting: null }), reload });
		expect(update).toHaveBeenCalledOnce();
		expect(states).toEqual(['activated']);
	});

	it('reloads at once when nothing newer was deployed', async () => {
		const reload = vi.fn();
		const registration = { update: async () => {}, installing: null, waiting: null };
		await updateNow({ container: container(registration), reload });
		expect(reload).toHaveBeenCalledOnce();
	});

	it('rejects without reloading when the newer build fails to install', async () => {
		const reload = vi.fn();
		const newer = worker('installing', 'redundant');
		const registration = { update: async () => {}, installing: newer, waiting: null };
		await expect(updateNow({ container: container(registration), reload })).rejects.toThrow(
			'didn’t install'
		);
		expect(reload).not.toHaveBeenCalled();
	});

	it('rejects without reloading when the newer build can’t be fetched', async () => {
		const reload = vi.fn();
		const registration = {
			update: () => Promise.reject(new TypeError('Failed to fetch')),
			installing: null,
			waiting: null
		};
		await expect(updateNow({ container: container(registration), reload })).rejects.toThrow(
			'Failed to fetch'
		);
		expect(reload).not.toHaveBeenCalled();
	});
});
