import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import path from "node:path";
import fs from "node:fs";

export const SOLID_COMMIT = "8a44c9eb0d8ae5d4a8193e2c88d891ee0ae7a82c";
export const SOLID_VERSION = "2.0.0-rc.1";
export const DOM_VERSION = "0.50.0-next.43";
export interface SolidProvenance {
	commit: string;
	checkoutPath: string;
	sourceTrees: Record<string, string>;
	sourceDigest: string;
	solidJs: string;
	web: string;
	signals: string;
	compiler: string;
	runtime: string;
}
export function validateSolidCheckout(): {
	root: string;
	provenance: SolidProvenance;
} {
	const root = path.resolve(
		process.env.MACRO_SOLID_CHECKOUT ?? "/Users/devagr/solid",
	);
	let commit: string;
	try {
		commit = execFileSync("git", ["-C", root, "rev-parse", "HEAD"], {
			encoding: "utf8",
		}).trim();
	} catch {
		throw new Error(`MPC401 required Solid checkout is unavailable at ${root}`);
	}
	if (commit !== SOLID_COMMIT)
		throw new Error(
			`MPC401 Solid checkout mismatch at ${root}: expected ${SOLID_COMMIT}, got ${commit}`,
		);
	const packagePaths = {
		"solid-js": "packages/solid",
		"@solidjs/web": "packages/solid-web",
		"@solidjs/signals": "packages/solid-signals",
	};
	const sourceTrees: Record<string, string> = {};
	for (const [name, sourcePath] of Object.entries(packagePaths)) {
		const status = execFileSync(
			"git",
			[
				"-C",
				root,
				"status",
				"--porcelain",
				"--untracked-files=no",
				"--",
				sourcePath,
			],
			{ encoding: "utf8" },
		).trim();
		if (status)
			throw new Error(
				`MPC402 tracked Solid source is modified at ${sourcePath}`,
			);
		sourceTrees[sourcePath] = execFileSync(
			"git",
			["-C", root, "rev-parse", `${commit}:${sourcePath}`],
			{ encoding: "utf8" },
		).trim();
		const pkg = JSON.parse(
			fs.readFileSync(path.join(root, sourcePath, "package.json"), "utf8"),
		);
		if (pkg.version !== SOLID_VERSION)
			throw new Error(`MPC403 ${name} version mismatch: ${pkg.version}`);
	}
	const sourceDigest = createHash("sha256")
		.update(
			Object.entries(sourceTrees)
				.sort()
				.map(([p, h]) => `${p}\0${h}\n`)
				.join(""),
		)
		.digest("hex");
	return {
		root,
		provenance: {
			commit,
			checkoutPath: root,
			sourceTrees,
			sourceDigest,
			solidJs: SOLID_VERSION,
			web: SOLID_VERSION,
			signals: SOLID_VERSION,
			compiler: DOM_VERSION,
			runtime: DOM_VERSION,
		},
	};
}
