export function glyphFor(kind: string): string {
  const k = kind.toLowerCase();
  if (k.includes('mcp')) return 'M';
  if (k.includes('wasm')) return 'W';
  if (k.includes('docker')) return 'D';
  if (k.includes('pipeline') || k.includes('chain')) return 'P';
  if (k.includes('native') || k.includes('builtin')) return 'N';
  return '·';
}
