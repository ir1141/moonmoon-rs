const TOKEN_RE = /^[A-Z2-7]{26,32}$/;
const ALPHA = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

export function isValidToken(t) {
  return typeof t === 'string' && TOKEN_RE.test(t);
}

export function generateToken(randomBytes) {
  const bytes = randomBytes(16);
  let bits = 0;
  let value = 0;
  let output = '';
  for (let i = 0; i < bytes.length; i++) {
    value = (value << 8) | bytes[i];
    bits += 8;
    while (bits >= 5) {
      output += ALPHA[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) {
    output += ALPHA[(value << (5 - bits)) & 31];
  }
  return output;
}
