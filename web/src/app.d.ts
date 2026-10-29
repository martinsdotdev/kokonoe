// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		interface PageState {
			/** A game file `/import` opened, which the Replay viewer shows. */
			imported?: import('#lib/game/replay.svelte.js').ReplaySource;
		}
		// interface Platform {}
	}
}

export {};
