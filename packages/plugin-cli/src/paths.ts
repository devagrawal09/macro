import path from "node:path";

/** Resolve a candidate and require it to remain strictly below an owned root. */
export function assertPathInside(root: string, candidate: string): string {
	const resolvedRoot = path.resolve(root);
	const resolvedCandidate = path.resolve(candidate);
	const relative = path.relative(resolvedRoot, resolvedCandidate);
	if (
		!relative ||
		relative.startsWith(`..${path.sep}`) ||
		relative === ".." ||
		path.isAbsolute(relative)
	) {
		throw new Error(
			`MPC601 output path escapes owned root: ${resolvedCandidate}`,
		);
	}
	return resolvedCandidate;
}

/** Resolve a release-relative path below an owned root. */
export function resolveOutputPath(root: string, relative: string): string {
	return assertPathInside(root, path.resolve(root, relative));
}
