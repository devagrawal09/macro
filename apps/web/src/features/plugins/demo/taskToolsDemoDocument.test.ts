import { describe, expect, it } from 'vitest';
import {
	buildTaskToolsDemoDocument,
	inlineClientBundle,
	escapeForScriptTag,
} from './taskToolsDemoDocument';

describe('task tools demo document', () => {
	it('rewrites the trailing export statement into a global entry assignment', () => {
		const output = inlineClientBundle('var a=1;\nexport{a as mount};\n');
		expect(output).toContain(
			'globalThis.__MACRO_PLUGIN_ENTRY__ = { mount: a };',
		);
		expect(output).not.toContain('export{');
	});

	it('fails closed when the bundle has no trailing export statement', () => {
		expect(() => inlineClientBundle('var a=1;\n')).toThrow();
	});

	it('escapes script-closing sequences inside embedded code', () => {
		const escaped = escapeForScriptTag('var s="</script>";');
		expect(escaped).toContain('<\\/script>');
		expect(escaped).not.toContain('</script>');
	});

	it('builds a complete srcdoc document for the committed Task Tools bundle', () => {
		const document = buildTaskToolsDemoDocument();
		expect(document).toMatch(/^<!doctype html>/i);
		expect(document).toContain('<div id="root"></div>');
		expect(document).toContain('macro.plugin.ready.v1');
		expect(document).toContain('__MACRO_PLUGIN_ENTRY__');
	});
});
