import { useEffect } from 'react';
import { CircleCheck, FileText, Printer, ShoppingCart } from 'lucide-react';
import type { SaleDetail } from '@/lib/api';
import { money } from '@/lib/format';
import { useSettings } from '@/lib/session';
import { Button, Modal } from '@/components/ui';
import { Receipt } from '@/components/Receipt';
import { useReceiptPrinter } from '@/components/useReceiptPrinter';

/** Shown after a sale: preview, print, start the next bill. */
export function ReceiptDialog({ sale, onClose, reprint }: { sale: SaleDetail | null; onClose: () => void; reprint?: boolean }) {
  const settings = useSettings();
  const printSale = useReceiptPrinter();

  useEffect(() => {
    if (!sale) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === 'p' && !e.ctrlKey && !(e.target instanceof HTMLInputElement)) {
        e.preventDefault();
        void printSale(sale, { reprint });
      }
      if (e.key === 'Enter') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [sale, printSale, onClose, reprint]);

  if (!sale) return null;
  const paper = settings.receiptPaper === 'a4' ? '80mm' : settings.receiptPaper;
  return (
    <Modal
      open
      onClose={onClose}
      size="md"
      title={reprint ? `Receipt ${sale.invoiceNo}` : 'Sale completed'}
      description={reprint ? undefined : `${sale.invoiceNo} · ${money(sale.total)}`}
      icon={reprint ? <FileText className="h-5 w-5" /> : <CircleCheck className="h-5 w-5 text-emerald-600" />}
      footer={
        <>
          <Button variant="ghost" className="mr-auto" icon={<FileText className="h-4 w-4" />} onClick={() => void printSale(sale, { reprint, a4: true })}>
            A4 invoice
          </Button>
          <Button icon={<Printer className="h-4 w-4" />} onClick={() => void printSale(sale, { reprint })}>
            Print <span className="kbd ml-1">P</span>
          </Button>
          <Button variant="primary" icon={<ShoppingCart className="h-4 w-4" />} onClick={onClose} data-autofocus>
            {reprint ? 'Close' : 'New sale'} <span className="kbd ml-1 border-brand-400 bg-brand-500 text-white">Enter</span>
          </Button>
        </>
      }
    >
      {!reprint && sale.changeAmount > 0 && (
        <div className="mb-4 flex items-center justify-between rounded-xl bg-emerald-50 px-5 py-3 ring-1 ring-inset ring-emerald-200">
          <span className="text-sm font-medium text-emerald-800">Give change</span>
          <span className="num text-3xl font-semibold text-emerald-700">{money(sale.changeAmount)}</span>
        </div>
      )}
      <div className="max-h-[52vh] overflow-y-auto rounded-xl bg-slate-100 p-4">
        <div className="mx-auto w-fit bg-white shadow-card">
          <Receipt sale={sale} settings={settings} paper={paper} reprint={reprint} />
        </div>
      </div>
    </Modal>
  );
}
