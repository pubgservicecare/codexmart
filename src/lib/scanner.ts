import { useEffect, useRef } from 'react';

/**
 * Detects USB barcode scanners that behave like keyboards: they "type" the
 * code very quickly and finish with Enter. Characters typed by a human are
 * much slower, so only bursts faster than `maxGapMs` per key are treated as
 * scans. Works whether or not a text box has focus, so the cashier never
 * has to click before scanning.
 */
export function useBarcodeScanner(onScan: (code: string) => void, enabled = true, maxGapMs = 35, minLength = 4) {
  const handler = useRef(onScan);
  handler.current = onScan;

  useEffect(() => {
    if (!enabled) return;
    let buffer = '';
    let last = 0;
    const onKey = (e: KeyboardEvent) => {
      const now = performance.now();
      if (now - last > maxGapMs) buffer = '';
      last = now;
      if (e.key === 'Enter') {
        if (buffer.length >= minLength) {
          const code = buffer;
          buffer = '';
          e.preventDefault();
          e.stopPropagation();
          // If the scan landed in a text box, remove the typed characters.
          const el = document.activeElement;
          if (el instanceof HTMLInputElement && el.value.endsWith(code)) {
            const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
            setter?.call(el, el.value.slice(0, -code.length));
            el.dispatchEvent(new Event('input', { bubbles: true }));
          }
          handler.current(code);
        }
        buffer = '';
        return;
      }
      if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) buffer += e.key;
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [enabled, maxGapMs, minLength]);
}
