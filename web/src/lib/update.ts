/**
 * Activates a newer deployed build at once and reloads into it, for the server's 4001 "update
 * required" close (spec §5.1), which arrives in M2. The service worker skips waiting, so a newer
 * build activates as soon as it has installed. With no service worker, or nothing newer, it only
 * reloads. It rejects, and doesn't reload, when the newer build can't be fetched or installed:
 * reloading into the old one would only meet the same close again.
 */
export async function updateNow({
	container,
	reload = () => location.reload()
}: { container?: ServiceWorkerContainer; reload?: () => void } = {}): Promise<void> {
	// Missing where the page isn't a secure context.
	const workers: ServiceWorkerContainer | undefined = container ?? navigator.serviceWorker;
	const registration = await workers?.getRegistration();
	await registration?.update();
	const newer = registration?.installing ?? registration?.waiting;
	if (newer) await activated(newer);
	reload();
}

function activated(worker: ServiceWorker): Promise<void> {
	return new Promise((resolve, reject) => {
		const check = () => {
			if (worker.state === 'activated') resolve();
			else if (worker.state === 'redundant') reject(new Error('The newer build didn’t install.'));
			else return;
			worker.removeEventListener('statechange', check);
		};
		worker.addEventListener('statechange', check);
		check();
	});
}
