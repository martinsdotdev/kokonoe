/** What a page says when it can't do what was asked: a Problem, in words for the player. */

/** A failure the page explains to the player, rather than only logging it. */
export class Problem extends Error {}

/** Throws `message` for the page to show, with the cause logged where a developer can see it. */
export const failed = (message: string) => (error: unknown) => {
	console.error(message, error);
	throw new Problem(message);
};

export const ENGINE = 'The engine didn’t load. Reload the page to try again.';

/** What the page says about `error`: a Problem's own words, or else to reload, logged as `what`. */
export const explain = (error: unknown, what: string): string => {
	if (error instanceof Problem) return error.message;
	console.error(what, error);
	return 'Something went wrong. Reload the page to try again.';
};
