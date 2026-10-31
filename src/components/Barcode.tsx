import { useEffect, useRef } from 'react';
import JsBarcode from 'jsbarcode';

function isEan13(v: string): boolean {
  if (!/^\d{13}$/.test(v)) return false;
  const digits = v.split('').map(Number);
  const sum = digits.slice(0, 12).reduce((s, d, i) => s + d * (i % 2 === 0 ? 1 : 3), 0);
  return (10 - (sum % 10)) % 10 === digits[12];
}

/** Barcode as SVG: EAN-13 for valid retail codes, Code 128 otherwise. */
export function Barcode({
  value,
  height = 40,
  width = 1.4,
  fontSize = 12,
  showText = true,
  className,
}: {
  value: string;
  height?: number;
  width?: number;
  fontSize?: number;
  showText?: boolean;
  className?: string;
}) {
  const ref = useRef<SVGSVGElement>(null);
  useEffect(() => {
    if (!ref.current || !value) return;
    try {
      JsBarcode(ref.current, value, {
        format: isEan13(value) ? 'EAN13' : 'CODE128',
        height,
        width,
        fontSize,
        displayValue: showText,
        margin: 0,
        background: 'transparent',
        lineColor: '#000',
        font: 'Inter Variable, Segoe UI, sans-serif',
        textMargin: 1,
      });
    } catch {
      // Characters Code 128 cannot encode: show nothing rather than fail.
      ref.current.innerHTML = '';
    }
  }, [value, height, width, fontSize, showText]);
  return <svg ref={ref} className={className} />;
}
