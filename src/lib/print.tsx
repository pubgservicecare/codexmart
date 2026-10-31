import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

/**
 * Printing through the Windows print dialog. The content is rendered into
 * #print-root, everything else is hidden by print CSS, and an @page rule
 * sets the paper size (80 mm / 58 mm rolls or A4).
 */

export type PaperSize = '80mm' | '58mm' | 'a4' | 'label-38x25' | 'label-50x25';

const PAGE_CSS: Record<PaperSize, string> = {
  '80mm': '@page { size: 80mm auto; margin: 0; }',
  '58mm': '@page { size: 58mm auto; margin: 0; }',
  a4: '@page { size: A4; margin: 12mm; }',
  'label-38x25': '@page { size: 38mm 25mm; margin: 0; }',
  'label-50x25': '@page { size: 50mm 25mm; margin: 0; }',
};

type PrintFn = (content: ReactNode, paper: PaperSize) => Promise<void>;
const PrintContext = createContext<PrintFn>(async () => {});

export function PrintProvider({ children }: { children: ReactNode }) {
  const [content, setContent] = useState<ReactNode>(null);

  const print = useCallback<PrintFn>(async (node, paper) => {
    setContent(node);
    const style = document.createElement('style');
    style.id = 'cxm-page-style';
    style.textContent = PAGE_CSS[paper];
    document.head.appendChild(style);
    // Let React render the content and barcodes draw before printing.
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(r, 60))));
    document.body.classList.add('printing');
    const cleanup = () => {
      document.body.classList.remove('printing');
      style.remove();
      setContent(null);
      window.removeEventListener('afterprint', cleanup);
    };
    window.addEventListener('afterprint', cleanup);
    window.print();
    // Fallback if afterprint is not delivered.
    setTimeout(() => {
      if (document.body.classList.contains('printing')) cleanup();
    }, 60_000);
  }, []);

  const root = document.getElementById('print-root');
  return (
    <PrintContext.Provider value={print}>
      {children}
      {root && content ? createPortal(content, root) : null}
    </PrintContext.Provider>
  );
}

export function usePrint(): PrintFn {
  return useContext(PrintContext);
}
