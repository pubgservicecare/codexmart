import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from 'react';
import { CircleAlert, CircleCheck, Info, X } from 'lucide-react';
import { errorMessage } from '@/lib/api';
import { cx } from '@/lib/utils';

type Kind = 'success' | 'error' | 'info';
interface Toast {
  id: number;
  kind: Kind;
  title: string;
  message?: string;
}

interface ToastApi {
  success: (title: string, message?: string) => void;
  error: (titleOrError: unknown, message?: string) => void;
  info: (title: string, message?: string) => void;
}

const ToastCtx = createContext<ToastApi | null>(null);
let seq = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const push = useCallback((kind: Kind, title: string, message?: string) => {
    const id = ++seq;
    setToasts((t) => [...t.slice(-2), { id, kind, title, message }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === 'error' ? 6000 : 3000);
  }, []);

  const api = useMemo<ToastApi>(
    () => ({
      success: (t, m) => push('success', t, m),
      info: (t, m) => push('info', t, m),
      error: (e, m) =>
        typeof e === 'string' ? push('error', e, m) : push('error', 'Could not complete the action', errorMessage(e)),
    }),
    [push],
  );

  return (
    <ToastCtx.Provider value={api}>
      {children}
      {/* Top right, below the header: clear of the Pay button and dialog footers. */}
      <div className="pointer-events-none fixed right-5 top-14 z-[60] flex w-96 flex-col gap-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            role="status"
            className={cx(
              'pointer-events-auto flex items-start gap-3 rounded-xl border bg-white p-3.5 shadow-pop',
              t.kind === 'error' ? 'border-rose-200' : t.kind === 'success' ? 'border-emerald-200' : 'border-slate-200',
            )}
          >
            {t.kind === 'success' && <CircleCheck className="mt-0.5 h-5 w-5 shrink-0 text-emerald-600" />}
            {t.kind === 'error' && <CircleAlert className="mt-0.5 h-5 w-5 shrink-0 text-rose-600" />}
            {t.kind === 'info' && <Info className="mt-0.5 h-5 w-5 shrink-0 text-brand-600" />}
            <div className="min-w-0 flex-1">
              <p className="text-sm font-semibold text-slate-900">{t.title}</p>
              {t.message && <p className="mt-0.5 break-words text-sm text-slate-600">{t.message}</p>}
            </div>
            <button
              className="text-slate-400 hover:text-slate-600"
              onClick={() => setToasts((all) => all.filter((x) => x.id !== t.id))}
              aria-label="Dismiss"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}

export function useToast(): ToastApi {
  const v = useContext(ToastCtx);
  if (!v) throw new Error('useToast outside ToastProvider');
  return v;
}
