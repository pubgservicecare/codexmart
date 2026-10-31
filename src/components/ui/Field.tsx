import {
  createContext,
  forwardRef,
  useContext,
  useEffect,
  useId,
  useState,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from 'react';
import { cx } from '@/lib/utils';
import { moneyInput, parseMoney, parseQty, qtyInput } from '@/lib/format';

/** Lets the control inside a Field pick up the id its <label> points to. */
const FieldId = createContext<string | undefined>(undefined);

export function Field({
  label,
  hint,
  error,
  required,
  children,
  className,
}: {
  label?: ReactNode;
  hint?: ReactNode;
  error?: string | null;
  required?: boolean;
  children: ReactNode;
  className?: string;
}) {
  const id = useId();
  return (
    <div className={className}>
      {label && (
        <label className="label" htmlFor={id}>
          {label}
          {required && <span className="ml-0.5 text-rose-500">*</span>}
        </label>
      )}
      <FieldId.Provider value={id}>{children}</FieldId.Provider>
      {error ? <p className="mt-1 text-2xs font-medium text-rose-600">{error}</p> : hint ? <p className="hint">{hint}</p> : null}
    </div>
  );
}

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement> & { invalid?: boolean }>(
  function Input({ className, invalid, id, ...rest }, ref) {
    const fieldId = useContext(FieldId);
    return <input ref={ref} id={id ?? fieldId} className={cx('input', invalid && 'input-invalid', className)} {...rest} />;
  },
);

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function Textarea(
  { className, rows = 3, id, ...rest },
  ref,
) {
  const fieldId = useContext(FieldId);
  return <textarea ref={ref} id={id ?? fieldId} rows={rows} className={cx('input resize-none', className)} {...rest} />;
});

export function Select({ className, children, id, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  const fieldId = useContext(FieldId);
  return (
    <select id={id ?? fieldId} className={cx('input cursor-pointer pr-8', className)} {...rest}>
      {children}
    </select>
  );
}

/** Text input that edits an amount in paisa. `null` = empty. */
export const MoneyInput = forwardRef<
  HTMLInputElement,
  Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange'> & {
    value: number | null;
    onChange: (v: number | null) => void;
    invalid?: boolean;
  }
>(function MoneyInput({ value, onChange, className, invalid, id, ...rest }, ref) {
  const fieldId = useContext(FieldId);
  const [text, setText] = useState(moneyInput(value));
  useEffect(() => {
    if (parseMoney(text) !== value) setText(moneyInput(value));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);
  return (
    <input
      ref={ref}
      id={id ?? fieldId}
      inputMode="decimal"
      className={cx('input num text-right', invalid && 'input-invalid', className)}
      value={text}
      onChange={(e) => {
        const t = e.target.value;
        if (t !== '' && !/^[\d,]*\.?\d{0,2}$/.test(t)) return;
        setText(t);
        onChange(parseMoney(t));
      }}
      onFocus={(e) => e.target.select()}
      {...rest}
    />
  );
});

/** Text input that edits a quantity in thousandths. */
export const QtyInput = forwardRef<
  HTMLInputElement,
  Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange'> & {
    value: number | null;
    onChange: (v: number | null) => void;
    allowDecimal?: boolean;
    invalid?: boolean;
  }
>(function QtyInput({ value, onChange, allowDecimal = true, className, invalid, id, ...rest }, ref) {
  const fieldId = useContext(FieldId);
  const [text, setText] = useState(value === null ? '' : qtyInput(value));
  useEffect(() => {
    if (parseQty(text, allowDecimal) !== value) setText(value === null ? '' : qtyInput(value));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);
  return (
    <input
      ref={ref}
      id={id ?? fieldId}
      inputMode="decimal"
      className={cx('input num text-right', invalid && 'input-invalid', className)}
      value={text}
      onChange={(e) => {
        const t = e.target.value;
        const re = allowDecimal ? /^\d*\.?\d{0,3}$/ : /^\d*$/;
        if (t !== '' && !re.test(t)) return;
        setText(t);
        onChange(parseQty(t, allowDecimal));
      }}
      onFocus={(e) => e.target.select()}
      {...rest}
    />
  );
});

export function Switch({
  checked,
  onChange,
  label,
  description,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
}) {
  return (
    <label className={cx('flex cursor-pointer items-start gap-3', disabled && 'cursor-not-allowed opacity-60')}>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={cx(
          'relative mt-0.5 inline-flex h-5 w-9 shrink-0 rounded-full transition focus-visible:ring-2 focus-visible:ring-brand-500/40',
          checked ? 'bg-brand-600' : 'bg-slate-300',
        )}
      >
        <span
          className={cx(
            'absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all',
            checked ? 'left-[18px]' : 'left-0.5',
          )}
        />
      </button>
      <span>
        <span className="block text-sm font-medium text-slate-800">{label}</span>
        {description && <span className="mt-0.5 block text-xs text-slate-500">{description}</span>}
      </span>
    </label>
  );
}

export function Checkbox({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: ReactNode }) {
  return (
    <label className="inline-flex cursor-pointer items-center gap-2 text-sm text-slate-700">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="h-4 w-4 rounded border-slate-300 accent-brand-600"
      />
      {label}
    </label>
  );
}
