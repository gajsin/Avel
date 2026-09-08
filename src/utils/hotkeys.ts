import type { ShortcutSpec } from '../types';

const KEY_ALIASES: Record<string, string> = {
  up: 'ArrowUp',
  arrowup: 'ArrowUp',
  down: 'ArrowDown',
  arrowdown: 'ArrowDown',
  left: 'ArrowLeft',
  arrowleft: 'ArrowLeft',
  right: 'ArrowRight',
  arrowright: 'ArrowRight',
  pageup: 'PageUp',
  pagedown: 'PageDown',
  home: 'Home',
  end: 'End',
  space: 'Space',
  enter: 'Enter',
  tab: 'Tab',
  backspace: 'Backspace',
  del: 'Delete',
  delete: 'Delete',
  ins: 'Insert',
  insert: 'Insert',
  esc: 'Escape',
  escape: 'Escape',
  equal: 'Equal',
  '=': 'Equal',
  minus: 'Minus',
  '-': 'Minus',
  bracketleft: 'BracketLeft',
  '[': 'BracketLeft',
  bracketright: 'BracketRight',
  ']': 'BracketRight',
  backslash: 'Backslash',
  '\\': 'Backslash',
  semicolon: 'Semicolon',
  ';': 'Semicolon',
  quote: 'Quote',
  "'": 'Quote',
  comma: 'Comma',
  ',': 'Comma',
  period: 'Period',
  '.': 'Period',
  slash: 'Slash',
  '/': 'Slash',
  backquote: 'Backquote',
  '`': 'Backquote',
  numpad0: 'Numpad0',
  numpad1: 'Numpad1',
  numpad2: 'Numpad2',
  numpad3: 'Numpad3',
  numpad4: 'Numpad4',
  numpad5: 'Numpad5',
  numpad6: 'Numpad6',
  numpad7: 'Numpad7',
  numpad8: 'Numpad8',
  numpad9: 'Numpad9',
  numpadadd: 'NumpadAdd',
  numpadsubtract: 'NumpadSubtract',
  numpadmultiply: 'NumpadMultiply',
  numpaddivide: 'NumpadDivide',
  numpaddecimal: 'NumpadDecimal',
  numpadenter: 'NumpadEnter',
  numpadequal: 'NumpadEqual',
};

export function normalizeKeyCode(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return '';
  const lower = trimmed.toLowerCase();

  if (Object.prototype.hasOwnProperty.call(KEY_ALIASES, lower)) {
    return KEY_ALIASES[lower];
  }
  const fMatch = lower.match(/^f([1-9]|1[0-9]|2[0-4])$/);
  if (fMatch) {
    return `F${fMatch[1]}`;
  }
  if (trimmed.length === 1 && /[a-z]/i.test(trimmed)) {
    return `Key${trimmed.toUpperCase()}`;
  }
  if (trimmed.length === 1 && /[0-9]/.test(trimmed)) {
    return `Digit${trimmed}`;
  }
  const keyMatch = lower.match(/^key([a-z])$/);
  if (keyMatch) {
    return `Key${keyMatch[1].toUpperCase()}`;
  }
  const digitMatch = lower.match(/^digit([0-9])$/);
  if (digitMatch) {
    return `Digit${digitMatch[1]}`;
  }
  return trimmed;
}

export function parseShortcutSpec(strOrJson: string, fallbackCode: string = ''): ShortcutSpec {
  if (!strOrJson || strOrJson === 'none' || strOrJson === 'unset') {
    return { code: fallbackCode || '', ctrl: false, alt: false, shift: false, meta: false };
  }
  if (strOrJson.startsWith('{')) {
    try {
      const parsed = JSON.parse(strOrJson);
      if (parsed && typeof parsed.code === 'string') {
        return {
          code: parsed.code || fallbackCode || '',
          ctrl: Boolean(parsed.ctrl),
          alt: Boolean(parsed.alt),
          shift: Boolean(parsed.shift),
          meta: Boolean(parsed.meta),
        };
      }
    } catch {
      // ignore
    }
  }

  // Parse legacy string e.g. "Ctrl+Alt+D", "F8", "ArrowDown", "PageUp"
  const parts = strOrJson.split('+').map((p) => p.trim());
  let ctrl = false;
  let alt = false;
  let shift = false;
  let meta = false;
  let code = '';

  for (const p of parts) {
    const lower = p.toLowerCase();
    if (lower === 'ctrl' || lower === 'control') ctrl = true;
    else if (lower === 'alt') alt = true;
    else if (lower === 'shift') shift = true;
    else if (lower === 'win' || lower === 'meta' || lower === 'cmd' || lower === 'command') meta = true;
    else {
      code = normalizeKeyCode(p);
    }
  }

  return {
    code: code || fallbackCode || '',
    ctrl,
    alt,
    shift,
    meta,
  };
}

const CODE_TO_DISPLAY: Record<string, string> = {
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Equal: '=',
  Minus: '-',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backquote: '`',
  Space: 'Space',
  Enter: 'Enter',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  Escape: 'Esc',
};

export function formatShortcutDisplay(spec: ShortcutSpec | null): string {
  if (!spec || !spec.code) return '';
  const parts: string[] = [];
  if (spec.ctrl) parts.push('Ctrl');
  if (spec.alt) parts.push('Alt');
  if (spec.shift) parts.push('Shift');
  if (spec.meta) parts.push('Win');

  let keyPart = '';
  if (spec.code.startsWith('Key')) {
    keyPart = spec.code.replace('Key', '');
  } else if (spec.code.startsWith('Digit')) {
    keyPart = spec.code.replace('Digit', '');
  } else if (spec.code.startsWith('Numpad')) {
    keyPart = spec.code.replace('Numpad', 'Num ');
  } else if (Object.prototype.hasOwnProperty.call(CODE_TO_DISPLAY, spec.code)) {
    keyPart = CODE_TO_DISPLAY[spec.code];
  } else {
    keyPart = spec.code;
  }

  parts.push(keyPart);
  return parts.join(' + ');
}

export function areSpecsEqual(a: ShortcutSpec, b: ShortcutSpec): boolean {
  return (
    a.code.toLowerCase() === b.code.toLowerCase() &&
    a.ctrl === b.ctrl &&
    a.alt === b.alt &&
    a.shift === b.shift &&
    a.meta === b.meta
  );
}
