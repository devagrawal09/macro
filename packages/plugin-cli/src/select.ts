import ts from "typescript";
import type { LoadedEntry, LoadedPlugin } from "./config";
function fail(
	code: string,
	message: string,
	node: ts.Node,
	sf: ts.SourceFile,
): never {
	const p = sf.getLineAndCharacterOfPosition(node.getStart(sf));
	throw new Error(
		`${code} ${sf.fileName}:${p.line + 1}:${p.character + 1} ${message}`,
	);
}
function field(
	object: ts.ObjectLiteralExpression,
	name: string,
): ts.Expression | undefined {
	for (const property of object.properties)
		if (
			ts.isPropertyAssignment(property) &&
			(ts.isIdentifier(property.name) || ts.isStringLiteral(property.name)) &&
			property.name.text === name
		)
			return property.initializer;
}
export function selectedSource(
	plugin: LoadedPlugin,
	entry: LoadedEntry,
): string {
	const options: ts.CompilerOptions = {
		target: ts.ScriptTarget.ES2022,
		module: ts.ModuleKind.ESNext,
		moduleResolution: ts.ModuleResolutionKind.Bundler,
		jsx: ts.JsxEmit.Preserve,
		skipLibCheck: true,
	};
	const program = ts.createProgram([plugin.path], options),
		sourceFile = program.getSourceFile(plugin.path);
	if (!sourceFile) throw new Error(`MPC201 cannot parse ${plugin.path}`);
	const sf: ts.SourceFile = sourceFile;
	const checker = program.getTypeChecker();
	for (const statement of sf.statements) {
		const allowed =
			ts.isImportDeclaration(statement) ||
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
				"MPC209",
				"top-level executable statements and re-exports are forbidden",
				statement,
				sf,
			);
	}
	const exp = sf.statements.find(ts.isExportAssignment);
	if (
		!exp ||
		!ts.isCallExpression(exp.expression) ||
		!ts.isObjectLiteralExpression(exp.expression.arguments[0])
	)
		fail(
			"MPC202",
			"default export must directly call definePlugin({...})",
			exp ?? sf,
			sf,
		);
	const root = exp.expression.arguments[0] as ts.ObjectLiteralExpression;
	const list = field(
		root,
		entry.target === "client" ? "contributions" : "handlers",
	);
	if (!list || !ts.isArrayLiteralExpression(list))
		fail(
			"MPC203",
			"entry descriptors must be direct helper calls in static arrays",
			list ?? root,
			sf,
		);
	const selected = list.elements[entry.index];
	if (
		!selected ||
		!ts.isCallExpression(selected) ||
		!ts.isObjectLiteralExpression(selected.arguments[0])
	)
		fail(
			"MPC203",
			"entry descriptors must be direct helper calls in static arrays",
			selected ?? list,
			sf,
		);
	const object = selected.arguments[0] as ts.ObjectLiteralExpression;
	const callback = field(
		object,
		entry.target === "client" ? "render" : "handle",
	);
	if (
		!callback ||
		!(ts.isArrowFunction(callback) || ts.isFunctionExpression(callback))
	)
		fail(
			"MPC204",
			`${entry.target} callback must be an inline function`,
			callback ?? object,
			sf,
		);
	function rejectRuntimeSyntax(node: ts.Node): void {
		if (
			ts.isCallExpression(node) &&
			node.expression.kind === ts.SyntaxKind.ImportKeyword
		)
			fail("MPC207", "dynamic import() is forbidden", node, sf);
		if (
			ts.isCallExpression(node) &&
			ts.isIdentifier(node.expression) &&
			["eval", "Function"].includes(node.expression.text)
		)
			fail("MPC208", `${node.expression.text} is forbidden`, node, sf);
		if (
			ts.isNewExpression(node) &&
			ts.isIdentifier(node.expression) &&
			["Function", "Worker", "SharedWorker", "WebAssembly"].includes(
				node.expression.text,
			)
		)
			fail("MPC208", `new ${node.expression.text} is forbidden`, node, sf);
		ts.forEachChild(node, rejectRuntimeSyntax);
	}
	rejectRuntimeSyntax(callback);
	const imports = new Set<ts.ImportDeclaration>(),
		constants = new Set<ts.VariableStatement>(),
		visiting = new Set<ts.Node>();
	function collect(node: ts.Node): void {
		if (visiting.has(node)) return;
		visiting.add(node);
		function visit(child: ts.Node): void {
			if (ts.isIdentifier(child)) {
				const symbol = checker.getSymbolAtLocation(child);
				for (const declaration of symbol?.declarations ?? []) {
					let current: ts.Node | undefined = declaration;
					while (
						current &&
						!ts.isImportDeclaration(current) &&
						!ts.isVariableStatement(current) &&
						current.parent !== sf
					)
						current = current.parent;
					if (current && ts.isImportDeclaration(current)) {
						if (
							ts.isStringLiteral(current.moduleSpecifier) &&
							current.moduleSpecifier.text !== "@macro/plugin"
						)
							imports.add(current);
					} else if (
						current &&
						ts.isVariableStatement(current) &&
						current.parent === sf
					) {
						if (!(current.declarationList.flags & ts.NodeFlags.Const))
							fail(
								"MPC205",
								`selected callback captures mutable module binding ${child.text}`,
								child,
								sf,
							);
						constants.add(current);
						for (const declaration of current.declarationList.declarations)
							if (declaration.initializer) collect(declaration.initializer);
					} else if (
						current?.parent === sf &&
						(ts.isFunctionDeclaration(current) ||
							ts.isClassDeclaration(current) ||
							ts.isEnumDeclaration(current))
					)
						fail(
							"MPC206",
							`move module-local ${child.text} to an imported pure helper`,
							child,
							sf,
						);
				}
			}
			ts.forEachChild(child, visit);
		}
		ts.forEachChild(node, visit);
	}
	collect(callback);
	const prefix = [...imports]
		.sort((a, b) => a.pos - b.pos)
		.map((n) => n.getText(sf))
		.concat(
			[...constants].sort((a, b) => a.pos - b.pos).map((n) => n.getText(sf)),
		)
		.join("\n");
	const fn = callback.getText(sf);
	return entry.target === "client"
		? `${prefix}\nimport {render as __solidRender} from "@solidjs/web";const __selected=${fn};export function mount(root,context){return __solidRender(()=>__selected(context),root)}`
		: `${prefix}\nconst __selected=${fn};export default __selected;export {__selected as handle}`;
}
