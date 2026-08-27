import { describe, expect, it } from 'vitest';
import { formatImperial, parseImperial } from './imperial';

describe('imperial boundary', () => {
  it.each([['24',384],['24"',384],["2'",384],["2' 6\"",480],['30 1/2"',488],['12 1/2"',200],['1/16"',1]])('parses %s exactly', (text, units) => expect(parseImperial(text)).toEqual({ ok: true, sixteenths: units }));
  it('rejects finer than one sixteenth without rounding', () => expect(parseImperial('1 1/32"')).toMatchObject({ ok: false }));
  it('formats exact common fractions', () => expect(formatImperial(200)).toBe('1\' 1/2"'));
});
