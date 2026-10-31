import { useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { cx } from '@/lib/utils';

/** Open dialogs, innermost last. */
const openModals: number[] = [];
let modalSeq = 0;

/** True while any dialog is open (page shortcuts should pause). */
export function isModalOpen(): boolean {
  return openModals.length > 0;
}

const SIZES = {
  sm: 'max-w-md',
  md: 'max-w-xl',
  lg: 'max-w-3xl',
  xl: 'max-w-5xl',
  full: 'max-w-[1400px]',
};

export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  size = 'md',
  closeOnBackdrop = true,
  icon,
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  size?: keyof typeof SIZES;
  closeOnBackdrop?: boolean;
  icon?: ReactNode;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    const id = ++modalSeq;
    openModals.push(id);
    const onKey = (e: KeyboardEvent) => {
      // Only the top-most dialog reacts, so Escape closes one layer at a time.
      if (e.key === 'Escape' && openModals[openModals.length - 1] === id) {
        e.stopPropagation();
        closeRef.current();
      }
    };
    window.addEventListener('keydown', onKey);
    const prev = document.activeElement as HTMLElement | null;
    // Focus the first field so keyboard users can start typing immediately.
    requestAnimationFrame(() => {
      const first = panel.current?.querySelector<HTMLElement>(
        '[data-autofocus], input:not([type=hidden]):not([disabled]), select, textarea',
      );
      (first ?? panel.current)?.focus();
    });
    return () => {
      window.removeEventListener('keydown', onKey);
      const idx = openModals.indexOf(id);
      if (idx >= 0) openModals.splice(idx, 1);
      prev?.focus?.();
    };
  }, [open]);

  if (!open) return null;
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-slate-900/40 p-4 pt-[6vh] backdrop-blur-[2px]">
      <div className="fixed inset-0" onClick={closeOnBackdrop ? onClose : undefined} aria-hidden />
      <div
        ref={panel}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        className={cx('relative mb-10 w-full rounded-2xl bg-white shadow-pop outline-none', SIZES[size])}
      >
        <div className="flex items-start gap-3 border-b border-slate-100 px-6 py-4">
          {icon && (
            <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-brand-50 text-brand-600">{icon}</div>
          )}
          <div className="min-w-0 flex-1">
            <h2 className="text-base font-semibold text-slate-900">{title}</h2>
            {description && <p className="mt-0.5 text-sm text-slate-500">{description}</p>}
          </div>
          <button onClick={onClose} className="btn btn-ghost btn-icon btn-sm -mr-2" aria-label="Close">
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="px-6 py-5">{children}</div>
        {footer && (
          <div className="flex items-center justify-end gap-2 rounded-b-2xl border-t border-slate-100 bg-slate-50/60 px-6 py-3.5">
            {footer}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
