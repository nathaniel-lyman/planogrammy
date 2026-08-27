export type ParseResult = { ok: true; sixteenths: number } | { ok: false; error: string };

function parseInches(value: string): ParseResult {
  const normalized = value.trim().replace(/"$/, '').trim();
  if (!normalized) return { ok: true, sixteenths: 0 };
  const wholeMatch = normalized.match(/^(\d+)$/);
  const fractionMatch = normalized.match(/^(?:(\d+)\s+)?(\d+)\s*\/\s*(\d+)$/);
  if (!wholeMatch && !fractionMatch) return { ok: false, error: 'Use feet, inches, and a common fraction (for example 2\' 6" or 30 1/2").' };
  const match = wholeMatch ? undefined : fractionMatch;
  const whole = wholeMatch ? Number(wholeMatch[1]) : Number(match?.[1] ?? 0);
  const numerator = match ? Number(match[2]) : 0;
  const denominator = match ? Number(match[3]) : 1;
  if (denominator === 0 || numerator >= denominator) return { ok: false, error: 'Enter a proper fraction with a non-zero denominator.' };
  const fractionalUnits = numerator * 16 / denominator;
  if (!Number.isInteger(fractionalUnits)) return { ok: false, error: 'Precision finer than 1/16 inch is not supported.' };
  return { ok: true, sixteenths: whole * 16 + fractionalUnits };
}

export function parseImperial(input: string): ParseResult {
  const value = input.trim();
  if (!value) return { ok: false, error: 'Enter an elevation.' };
  const feetMatch = value.match(/^(\d+)\s*'\s*(.*)$/);
  if (!feetMatch) return parseInches(value);
  const feet = Number(feetMatch[1]);
  const inches = parseInches(feetMatch[2]);
  return inches.ok ? { ok: true, sixteenths: feet * 192 + inches.sixteenths } : inches;
}

export function formatImperial(sixteenths: number): string {
  const sign = sixteenths < 0 ? '-' : '';
  let units = Math.abs(sixteenths);
  const feet = Math.floor(units / 192);
  units %= 192;
  const inches = Math.floor(units / 16);
  const fraction = units % 16;
  const gcd = (a: number, b: number): number => b ? gcd(b, a % b) : a;
  let fractional = '';
  if (fraction) {
    const divisor = gcd(fraction, 16);
    fractional = `${fraction / divisor}/${16 / divisor}`;
  }
  const inchValue = [inches || (!feet && !fraction ? 0 : ''), fractional].filter(value => value !== '').join(' ');
  const inchPart = inchValue ? `${inchValue}"` : '';
  return `${sign}${feet ? `${feet}'` : ''}${feet && inchPart ? ' ' : ''}${inchPart}`;
}
