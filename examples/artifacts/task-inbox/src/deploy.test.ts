import { expect, test } from 'bun:test';
import { overwriteUrl } from './deploy';
test('deploy overwrites only the configured fixed document', () => expect(overwriteUrl('http://storage/api/', 'doc id')).toBe('http://storage/api/documents/doc%20id/simple_save'));
