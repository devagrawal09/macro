import ts from "typescript";
import type { Plugin } from "vite";
const builtins = new Set([
	"assert",
	"buffer",
	"child_process",
	"cluster",
	"crypto",
	"dgram",
	"dns",
	"events",
	"fs",
	"http",
	"https",
	"module",
	"net",
	"os",
	"path",
	"perf_hooks",
	"process",
	"readline",
	"stream",
	"tls",
	"tty",
	"url",
	"util",
	"v8",
	"vm",
	"worker_threads",
	"zlib",
]);
export function projectPolicy(
	target: "client" | "server",
	root: string,
): Plugin {
	return {
		name: "macro-plugin-selected-policy",
		enforce: "pre",
		resolveId(source, importer) {
			const bare = source.replace(/^(?:node|bun):/, "");
			if (
				source.startsWith("node:") ||
				source.startsWith("bun:") ||
				builtins.has(bare) ||
				/\.node$/.test(source)
			)
				throw new Error(
					`MPC301 ${source} is forbidden in ${target} Plugin code${importer ? ` imported by ${importer}` : ""}`,
				);
			if (target === "client" && /(?:^|[/.])server(?:[/.]|$)/.test(source))
				throw new Error(
					`MPC302 server-only import ${source} is forbidden in client Plugin code`,
				);
			if (
				target === "server" &&
				(source === "solid-js" ||
					source.startsWith("@solidjs/web") ||
					/(?:^|[/.])client(?:[/.]|$)/.test(source))
			)
				throw new Error(
					`MPC302 client-only import ${source} is forbidden in server Plugin code`,
				);
		},
		transform(code, id) {
			if (!id.startsWith(root) || !/[.][cm]?[jt]sx?$/.test(id)) return null;
			const sf = ts.createSourceFile(
					id,
					code,
					ts.ScriptTarget.Latest,
					true,
					/tsx$/.test(id) ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
				),
				globals =
					target === "client"
						? new Set(["Bun", "process", "Deno", "require", "Buffer"])
						: new Set([
								"Bun",
								"process",
								"Deno",
								"require",
								"Buffer",
								"window",
								"document",
								"navigator",
								"Worker",
								"SharedWorker",
								"WebAssembly",
							]);
			function fail(code: string, message: string, node: ts.Node): never {
				const p = sf.getLineAndCharacterOfPosition(node.getStart(sf));
				throw new Error(
					`${code} ${id}:${p.line + 1}:${p.character + 1} ${message}`,
				);
			}
			for (const statement of sf.statements) {
				const allowed =
					ts.isImportDeclaration(statement) ||
					ts.isExportDeclaration(statement) ||
					ts.isExportAssignment(statement) ||
					ts.isVariableStatement(statement) ||
					ts.isFunctionDeclaration(statement) ||
					ts.isClassDeclaration(statement) ||
					ts.isInterfaceDeclaration(statement) ||
					ts.isTypeAliasDeclaration(statement) ||
					ts.isEnumDeclaration(statement) ||
					ts.isModuleDeclaration(statement);
				if (!allowed)
					fail(
						"MPC310",
						"top-level executable statements are forbidden in Plugin modules",
						statement,
					);
			}
			function visit(node: ts.Node): void {
				if (ts.isIdentifier(node) && globals.has(node.text)) {
					const parent = node.parent,
						isProperty =
							ts.isPropertyAccessExpression(parent) && parent.name === node,
						isKey =
							(ts.isPropertyAssignment(parent) ||
								ts.isMethodDeclaration(parent)) &&
							parent.name === node,
						isDeclaration =
							(ts.isVariableDeclaration(parent) ||
								ts.isParameter(parent) ||
								ts.isFunctionDeclaration(parent) ||
								ts.isClassDeclaration(parent)) &&
							parent.name === node;
					if (!isProperty && !isKey && !isDeclaration)
						fail(
							"MPC303",
							`${node.text} is forbidden in ${target} Plugin code`,
							node,
						);
				}
				if (
					ts.isCallExpression(node) &&
					node.expression.kind === ts.SyntaxKind.ImportKeyword
				)
					fail("MPC304", "dynamic import() is forbidden", node);
				if (
					ts.isCallExpression(node) &&
					ts.isIdentifier(node.expression) &&
					["eval", "Function"].includes(node.expression.text)
				)
					fail("MPC305", `${node.expression.text} is forbidden`, node);
				ts.forEachChild(node, visit);
			}
			visit(sf);
			return null;
		},
	};
}
export function assertBundle(
	target: "client" | "server",
	code: string,
	file: string,
): void {
	if (
		/\b(?:import|export)\s+(?:[^"']*?from\s*)?["']/.test(code) ||
		/\bimport\s*\(/.test(code)
	)
		throw new Error(`MPC306 ${file} contains an external import`);
	const markers =
		target === "client"
			? ["MPC_SERVER_ONLY", "@macro/plugin/server"]
			: ["MPC_CLIENT_ONLY", "@macro/plugin/client", "@solidjs/web", "solid-js"];
	for (const marker of markers)
		if (code.includes(marker))
			throw new Error(
				`MPC307 ${file} contains forbidden ${target} marker ${marker}`,
			);
	const forbidden =
		target === "client"
			? ["Bun.", "process.env", "node:", "bun:"]
			: [
					"Bun.",
					"process.env",
					"node:",
					"bun:",
					"window.",
					"document.",
					"navigator.",
					"WebAssembly.",
				];
	for (const value of forbidden)
		if (code.includes(value))
			throw new Error(
				`MPC308 ${file} contains forbidden ${target} runtime value ${value}`,
			);
	if (/\beval\s*\(|new Function\s*\(/.test(code))
		throw new Error(`MPC309 ${file} contains runtime code generation`);
}
