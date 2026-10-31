import { useEffect, useState } from 'react';
import { Percent } from 'lucide-react';
import type { Discount } from '@/lib/api';
import { Button, Field, Input, Modal } from '@/components/ui';
import { money, moneyInput, parseMoney } from '@/lib/format';
import { cx } from '@/lib/utils';

export function DiscountDialog({
  open,
  onClose,
  title,
  base,
  value,
  onApply,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  /** Amount the discount applies to, for the preview. */
  base: number;
  value: Discount | null;
  onApply: (d: Discount | null) => void;
}) {
  const [kind, setKind] = useState<Discount['kind']>('percent');
  const [text, setText] = useState('');

  useEffect(() => {
    if (!open) return;
    setKind(value?.kind ?? 'percent');
    setText(value ? (value.kind === 'percent' ? String(value.value / 100) : moneyInput(value.value)) : '');
  }, [open, value]);

  const parsed: Discount | null = (() => {
    if (text.trim() === '') return null;
    if (kind === 'percent') {
      const n = Number(text);
      if (!Number.isFinite(n) || n < 0 || n > 100) return null;
      return { kind, value: Math.round(n * 100) };
    }
    const m = parseMoney(text);
    return m === null || m < 0 ? null : { kind, value: m };
  })();
  const discountAmount = parsed ? (parsed.kind === 'percent' ? Math.round((base * parsed.value) / 10000) : parsed.value) : 0;
  const tooBig = discountAmount > base;

  const apply = () => {
    onApply(parsed && parsed.value > 0 ? parsed : null);
    onClose();
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      size="sm"
      icon={<Percent className="h-5 w-5" />}
      footer={
        <>
          {value && (
            <Button
              variant="ghost"
              className="mr-auto"
              onClick={() => {
                onApply(null);
                onClose();
              }}
            >
              Remove discount
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={(text !== '' && !parsed) || tooBig} onClick={apply}>
            Apply
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!tooBig && (text === '' || parsed)) apply();
        }}
        className="space-y-4"
      >
        <div className="grid grid-cols-2 gap-1 rounded-lg bg-slate-100 p-1">
          {(['percent', 'amount'] as const).map((k) => (
            <button
              key={k}
              type="button"
              className={cx('rounded-md py-1.5 text-sm font-medium', kind === k ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500')}
              onClick={() => setKind(k)}
            >
              {k === 'percent' ? 'Percent (%)' : 'Amount'}
            </button>
          ))}
        </div>
        <Field label={kind === 'percent' ? 'Discount %' : 'Discount amount'} error={tooBig ? 'The discount is more than the amount.' : null}>
          <Input autoFocus inputMode="decimal" className="num text-right text-lg" value={text} onChange={(e) => setText(e.target.value)} />
        </Field>
        <div className="rounded-lg bg-slate-50 px-3 py-2 text-sm text-slate-600">
          Applies to {money(base)} → discount <b className="num text-slate-900">{money(discountAmount)}</b>
        </div>
      </form>
    </Modal>
  );
}
