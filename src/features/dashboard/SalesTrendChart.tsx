import { useEffect, useRef, useState } from 'react';
import type { TrendPoint } from '@/lib/api';
import { compactMoney, money, shortDate } from '@/lib/format';

const HEIGHT = 220;
const PAD = { top: 12, right: 8, bottom: 26, left: 48 };
const BAR_COLOR = '#6366F1'; // brand-500, validated against the white card surface
const BAR_HOVER = '#4F46E5';

function niceStep(max: number): number {
  const raw = max / 4;
  const pow = 10 ** Math.floor(Math.log10(raw));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * pow >= raw) return m * pow;
  return 10 * pow;
}

/** Column with a 4px rounded data end and a square baseline. */
function columnPath(x: number, y: number, w: number, h: number): string {
  const r = Math.min(4, w / 2, h);
  return `M${x},${y + h} V${y + r} Q${x},${y} ${x + r},${y} H${x + w - r} Q${x + w},${y} ${x + w},${y + r} V${y + h} Z`;
}

export function SalesTrendChart({ points }: { points: TrendPoint[] }) {
  const wrap = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(600);
  const [hover, setHover] = useState<number | null>(null);

  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => setWidth(Math.max(280, e.contentRect.width)));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const values = points.map((p) => Math.max(p.netSales, 0));
  const max = Math.max(...values, 1);
  const step = niceStep(max);
  const top = Math.ceil(max / step) * step;
  const ticks = Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step);
  const plotW = width - PAD.left - PAD.right;
  const plotH = HEIGHT - PAD.top - PAD.bottom;
  const band = plotW / points.length;
  const barW = Math.min(24, band * 0.6);
  const y = (v: number) => PAD.top + plotH - (v / top) * plotH;
  const best = values.indexOf(Math.max(...values));

  return (
    <div ref={wrap} className="relative">
      <svg width={width} height={HEIGHT} role="img" aria-label="Net sales per day for the last 14 days" onMouseLeave={() => setHover(null)}>
        {ticks.map((t) => (
          <g key={t}>
            <line x1={PAD.left} x2={width - PAD.right} y1={y(t)} y2={y(t)} stroke="#E2E8F0" strokeWidth={1} />
            <text x={PAD.left - 8} y={y(t)} dy="0.32em" textAnchor="end" className="fill-slate-400 text-[10px]">
              {compactMoney(t)}
            </text>
          </g>
        ))}
        {points.map((p, i) => {
          const cx = PAD.left + band * i + band / 2;
          const v = values[i];
          const h = Math.max((v / top) * plotH, v > 0 ? 2 : 0);
          return (
            <g key={p.date} onMouseEnter={() => setHover(i)}>
              {/* Hit target: the whole band, larger than the mark. */}
              <rect x={PAD.left + band * i} y={PAD.top} width={band} height={plotH} fill="transparent" />
              {h > 0 && <path d={columnPath(cx - barW / 2, y(v), barW, h)} fill={hover === i ? BAR_HOVER : BAR_COLOR} />}
              {(i === 0 || i === points.length - 1 || i % 2 === (points.length - 1) % 2) && (
                <text x={cx} y={HEIGHT - 8} textAnchor="middle" className="fill-slate-400 text-[10px]">
                  {shortDate(p.date)}
                </text>
              )}
              {i === best && v > 0 && hover === null && (
                <text x={cx} y={y(v) - 6} textAnchor="middle" className="fill-slate-600 text-[10px] font-medium">
                  {compactMoney(v)}
                </text>
              )}
            </g>
          );
        })}
      </svg>
      {hover !== null && (
        <div
          className="pointer-events-none absolute z-10 w-44 rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs shadow-pop"
          style={{
            left: Math.min(Math.max(PAD.left + band * hover + band / 2 - 88, 0), width - 176),
            top: Math.max(y(values[hover]) - 78, 0),
          }}
        >
          <p className="font-semibold text-slate-900">{shortDate(points[hover].date)}</p>
          <div className="mt-1 flex justify-between text-slate-600">
            <span>Net sales</span>
            <span className="num font-medium text-slate-900">{money(points[hover].netSales)}</span>
          </div>
          <div className="flex justify-between text-slate-600">
            <span>Gross profit</span>
            <span className="num">{money(points[hover].grossProfit)}</span>
          </div>
          <div className="flex justify-between text-slate-600">
            <span>Bills</span>
            <span className="num">{points[hover].invoices}</span>
          </div>
        </div>
      )}
    </div>
  );
}
