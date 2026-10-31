import { useCallback } from 'react';
import { api, type SaleDetail } from '@/lib/api';
import { usePrint } from '@/lib/print';
import { useSettings } from '@/lib/session';
import { useToast } from '@/components/ui';
import { Receipt } from './Receipt';

/**
 * Prints a sale the way Settings say: through the Windows print dialog
 * (any printer, with preview) or straight to a thermal printer (ESC/POS).
 */
export function useReceiptPrinter() {
  const settings = useSettings();
  const print = usePrint();
  const toast = useToast();

  return useCallback(
    async (sale: SaleDetail, opts: { reprint?: boolean; a4?: boolean } = {}) => {
      try {
        if (settings.receiptPrintMode === 'escpos' && !opts.a4) {
          await api.printing.receiptDirect(sale.id, !!opts.reprint);
          toast.success('Receipt sent to printer', settings.receiptPrinter);
          return;
        }
        const paper = opts.a4 ? 'a4' : settings.receiptPaper;
        await print(<Receipt sale={sale} settings={settings} paper={paper} reprint={opts.reprint} />, paper);
      } catch (e) {
        toast.error(e);
      }
    },
    [settings, print, toast],
  );
}
