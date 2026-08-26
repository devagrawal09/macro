import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import {
	validateSolidCheckout,
	SOLID_COMMIT,
	DOM_VERSION,
	SOLID_VERSION,
} from "../src/solid";
test("records exact tracked Solid provenance", () => {
	const { provenance } = validateSolidCheckout();
	expect(provenance.commit).toBe(SOLID_COMMIT);
	expect(provenance.solidJs).toBe(SOLID_VERSION);
	expect(provenance.compiler).toBe(DOM_VERSION);
	expect(Object.keys(provenance.sourceTrees)).toEqual([
		"packages/solid",
		"packages/solid-web",
		"packages/solid-signals",
	]);
});
test("fails clearly when the checkout commit differs", async () => {
	const root = await mkdtemp(`${tmpdir()}/macro-solid-mismatch-`),
		old = process.env.MACRO_SOLID_CHECKOUT;
	try {
		Bun.spawnSync(["git", "init"], { cwd: root });
		Bun.spawnSync(
			[
				"git",
				"-c",
				"user.name=Test",
				"-c",
				"user.email=test@example.com",
				"commit",
				"--allow-empty",
				"-m",
				"wrong",
			],
			{ cwd: root },
		);
		process.env.MACRO_SOLID_CHECKOUT = root;
		expect(() => validateSolidCheckout()).toThrow(
			/MPC401 Solid checkout mismatch/,
		);
	} finally {
		if (old === undefined) delete process.env.MACRO_SOLID_CHECKOUT;
		else process.env.MACRO_SOLID_CHECKOUT = old;
		await rm(root, { recursive: true, force: true });
	}
});
