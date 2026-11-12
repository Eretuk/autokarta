/** Delays `fn` until calls stop for `ms`. `cancel()` drops a pending call,
 * e.g. when a scan replaces what was being typed. */
export function debounce<A extends unknown[]>(fn: (...args: A) => void, ms: number) {
	let timer: ReturnType<typeof setTimeout> | undefined;
	const call = (...args: A) => {
		clearTimeout(timer);
		timer = setTimeout(() => fn(...args), ms);
	};
	call.cancel = () => clearTimeout(timer);
	return call;
}
