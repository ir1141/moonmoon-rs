import { describe, test, expect } from 'bun:test';
import { isValidToken, generateToken } from '../static/lib/token.js';

describe('isValidToken', () => {
  test.each([
    ['non-string',           null,                       false],
    ['too short (25)',       'A'.repeat(25),             false],
    ['too long (33)',        'A'.repeat(33),             false],
    ['lowercase letter',     'a'.repeat(26),             false],
    ['digit outside 2-7',    'A'.repeat(25) + '0',       false],
    ['valid 26-char',        'ABCDEFGHIJKLMNOPQRSTUVWXYZ', true],
    ['valid 32-char',        'A'.repeat(32),             true],
  ])('%s', (_label, input, expected) => {
    expect(isValidToken(input)).toBe(expected);
  });
});

describe('generateToken', () => {
  test('deterministic input produces a 26-char base32 token that passes isValidToken', () => {
    const bytes = new Uint8Array(16);
    for (let i = 0; i < 16; i++) bytes[i] = (i * 17) & 0xff;
    const token = generateToken(() => bytes);
    expect(token).toHaveLength(26);
    expect(isValidToken(token)).toBe(true);
  });

  test('same input bytes produce same token (pure function)', () => {
    const bytes = new Uint8Array(16).fill(42);
    expect(generateToken(() => bytes)).toBe(generateToken(() => bytes));
  });

  test('all-zero input still produces a 26-char token in the alphabet', () => {
    const token = generateToken(() => new Uint8Array(16).fill(0));
    expect(token).toHaveLength(26);
    expect(token).toMatch(/^[A-Z2-7]+$/);
  });
});
