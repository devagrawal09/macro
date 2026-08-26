import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createServerPluginExecutor } from "../src/executor";

const repoRoot = path.resolve(import.meta.dir, "../../.."),
	cliPath = path.join(repoRoot, "packages/plugin-cli/src/cli.ts"),
	fixture = path.join(
		repoRoot,
		"packages/plugin-cli/test/fixtures/server/plugin.ts",
	);

function solidCheckoutAvailable(): boolean {
	try {
		const proc = Bun.spawnSync(["git", "-C", "/Users/devagr/solid", "rev-parse", "HEAD"], {
			stdout: "pipe",
			stderr: "pipe",
		});
		return proc.exitCode === 0;
	} catch {
		return false;
	}
}

test("executes a CLI-built server bundle end-to-end", async () => {
	if (!solidCheckoutAvailable()) {
		console.warn("skipping e2e: pinned Solid checkout unavailable");
		return;
	}
	const outDir = await mkdtemp(path.join(tmpdir(), "macro-plugin-e2e-"));
	try {
		const build = Bun.spawnSync([
			process.execPath,
			cliPath,
			"plugin",
			"build",
			fixture,
			"--out-dir",
			outDir,
		]);
		expect(build.exitCode).toBe(0);

		const manifest = JSON.parse(
			await readFile(path.join(outDir, "manifest.json"), "utf8"),
		);
		expect(manifest.plugin.id).toBe("com.macro.server");
		const serverEntry = Object.values<{ target: string; file: string }>(
			manifest.entrypoints,
		).find((entry) => entry.target === "server");
		expect(serverEntry?.file).toBe("server/received/index.js");

		const executor = createServerPluginExecutor();
		savedSlot().__saved = undefined;
		const outcome = await executor.invoke({
			bundlePath: path.join(outDir, serverEntry!.file),
			eventId: "evt-e2e-1",
			eventType: "document.created",
			expectedEventType: "document.created",
			event: { id: "doc-42" },
			projectId: "proj-local",
			installationId: "inst-local",
			capabilities: [],
		});
		expect(outcome.status).toBe("completed");

		// The fixture's save() helper proves the compiled bundle really ran:
		// it sets globalThis.__saved = `${projectId}:${event.id}`.
		expect(savedSlot().__saved).toBe("proj-local:doc-42");

		const [record] = executor.runs();
		expect(record?.status).toBe("completed");
		expect(record?.eventType).toBe("document.created");
	} finally {
		await rm(outDir, { recursive: true, force: true });
	}
});

// The fixture's save() helper writes `globalThis.__saved = \`\${project}:\${id}\``.
function savedSlot(): { __saved?: string } {
	return globalThis as unknown as { __saved?: string };
}
