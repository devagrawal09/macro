/** Small internal helpers shared by kit components. */

/** Joins optional class names, dropping empties. */
export function joinClass(...parts: Array<string | undefined>): string {
	return parts.filter((part) => part !== undefined && part !== "").join(" ");
}

let uid = 0;

/** Generates a stable-per-mount id for field associations. */
export function nextId(prefix: string): string {
	uid += 1;
	return `mui-${prefix}-${uid}`;
}

/** Resolves a Solid ref that may be a function or an object holder. */
export function setRef<T>(ref: unknown, value: T): void {
	if (typeof ref === "function") (ref as (value: T) => void)(value);
	else if (ref && typeof ref === "object") (ref as { current?: T }).current = value;
}
