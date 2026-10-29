import { describe, expect, it } from 'vitest';
import { formatBytes, formatEta, formatSpeed, groupDigits, percent, plural } from '../lib/format';

describe('format', () => {
  it('formats bytes in binary units', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1536)).toBe('1.50 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.00 MB');
    expect(formatBytes(-1)).toBe('—');
  });
  it('formats speed and eta', () => {
    expect(formatSpeed(0)).toBe('—');
    expect(formatSpeed(2 * 1024 * 1024)).toBe('2.00 MB/s');
    expect(formatEta(null)).toBe('—');
    expect(formatEta(45)).toBe('45s');
    expect(formatEta(125)).toBe('2m 5s');
    expect(formatEta(3720)).toBe('1h 2m');
  });
  it('clamps percent and handles zero totals', () => {
    expect(percent(5, 0)).toBe(0);
    expect(percent(150, 100)).toBe(100);
    expect(percent(25, 100)).toBe(25);
  });
  it('plural and code grouping', () => {
    expect(plural(1, 'file')).toBe('1 file');
    expect(plural(2, 'file')).toBe('2 files');
    expect(groupDigits('123456')).toBe('123 456');
  });
});
