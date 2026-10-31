import type { CSSProperties, ReactNode } from 'react';
import type { SaleDetail, Settings } from '@/lib/api';
import { amount, dateTime, methodLabel, qty } from '@/lib/format';
import { Barcode } from './Barcode';

type Paper = Settings['receiptPaper'];

function Row({ left, right, bold, big }: { left: ReactNode; right: ReactNode; bold?: boolean; big?: boolean }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, fontWeight: bold ? 700 : 400, fontSize: big ? '1.35em' : undefined }}>
      <span>{left}</span>
      <span style={{ fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap' }}>{right}</span>
    </div>
  );
}

const Rule = () => <div style={{ borderTop: '1px dashed #000', margin: '6px 0' }} />;

/**
 * Printable receipt. The same component is used for the on-screen preview
 * and for printing, so what the cashier sees is what prints.
 */
export function Receipt({ sale, settings, paper, reprint }: { sale: SaleDetail; settings: Settings; paper: Paper; reprint?: boolean }) {
  if (paper === 'a4') return <InvoiceA4 sale={sale} settings={settings} reprint={reprint} />;
  const narrow = paper === '58mm';
  const style: CSSProperties = {
    width: narrow ? '48mm' : '72mm',
    margin: '0 auto',
    padding: '3mm 0',
    fontFamily: '"Inter Variable", "Segoe UI", Arial, sans-serif',
    fontSize: narrow ? 10.5 : 12,
    lineHeight: 1.35,
    color: '#000',
    background: '#fff',
  };
  const m = (v: number) => amount(v, settings.currencyDecimals);
  return (
    <div style={style}>
      <div style={{ textAlign: 'center' }}>
        <div style={{ fontSize: '1.45em', fontWeight: 800, letterSpacing: '-0.01em' }}>{settings.storeName}</div>
        {settings.storeTagline && <div>{settings.storeTagline}</div>}
        {settings.storeAddress && <div style={{ whiteSpace: 'pre-line' }}>{settings.storeAddress}</div>}
        {settings.storePhone && <div>Tel: {settings.storePhone}</div>}
        {settings.storeTaxId && <div>Tax ID: {settings.storeTaxId}</div>}
        {settings.receiptHeader && <div style={{ whiteSpace: 'pre-line', marginTop: 4 }}>{settings.receiptHeader}</div>}
      </div>
      <Rule />
      {sale.status === 'void' && <div style={{ textAlign: 'center', fontWeight: 800 }}>*** VOID ***</div>}
      {reprint && sale.status !== 'void' && <div style={{ textAlign: 'center', fontWeight: 700 }}>DUPLICATE RECEIPT</div>}
      <Row left="Invoice" right={<b>{sale.invoiceNo}</b>} />
      <Row left="Date" right={dateTime(sale.createdAt)} />
      <Row left="Cashier" right={sale.cashierName} />
      {sale.customerName && <Row left="Customer" right={sale.customerName} />}
      <Rule />
      {sale.items.map((i) => (
        <div key={i.id} style={{ marginBottom: 4 }}>
          <div style={{ fontWeight: 600 }}>{i.productName}</div>
          <Row left={`${qty(i.qty)} ${i.unitName} × ${m(i.unitPrice)}`} right={m(i.grossAmount)} />
          {i.lineDiscount > 0 && <Row left="  Discount" right={`-${m(i.lineDiscount)}`} />}
        </div>
      ))}
      <Rule />
      <Row left="Subtotal" right={m(sale.subtotal)} />
      {sale.discountTotal > 0 && <Row left="Discount" right={`-${m(sale.discountTotal)}`} />}
      <div style={{ margin: '4px 0' }}>
        <Row left="TOTAL" right={`${settings.currencySymbol} ${m(sale.total)}`} bold big />
      </div>
      {sale.payments.map((p, idx) => (
        <Row key={idx} left={`Paid (${methodLabel(p.method)})`} right={m(p.amount)} />
      ))}
      {sale.tenderedCash > 0 && (
        <>
          <Row left="Cash received" right={m(sale.tenderedCash)} />
          <Row left="Change" right={m(sale.changeAmount)} bold />
        </>
      )}
      {sale.creditAmount > 0 && <Row left="Balance due (credit)" right={m(sale.creditAmount)} bold />}
      <div style={{ marginTop: 2 }}>Items: {sale.itemCount}</div>
      <Rule />
      {settings.receiptFooter && <div style={{ textAlign: 'center', whiteSpace: 'pre-line' }}>{settings.receiptFooter}</div>}
      <div style={{ display: 'flex', justifyContent: 'center', marginTop: 6 }}>
        <Barcode value={sale.invoiceNo} height={narrow ? 28 : 34} width={narrow ? 1.1 : 1.3} fontSize={narrow ? 9 : 10} />
      </div>
      <div style={{ textAlign: 'center', fontSize: '0.8em', marginTop: 4 }}>Powered by CODE X MART</div>
    </div>
  );
}

function InvoiceA4({ sale, settings, reprint }: { sale: SaleDetail; settings: Settings; reprint?: boolean }) {
  const m = (v: number) => amount(v, settings.currencyDecimals);
  const th: CSSProperties = { textAlign: 'left', borderBottom: '1.5px solid #000', padding: '6px 4px', fontSize: 11 };
  const td: CSSProperties = { borderBottom: '1px solid #ddd', padding: '6px 4px', fontSize: 12 };
  const num: CSSProperties = { textAlign: 'right', fontVariantNumeric: 'tabular-nums' };
  return (
    <div style={{ fontFamily: '"Inter Variable", "Segoe UI", Arial, sans-serif', color: '#000', fontSize: 12, background: '#fff' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div>
          <div style={{ fontSize: 22, fontWeight: 800 }}>{settings.storeName}</div>
          {settings.storeTagline && <div>{settings.storeTagline}</div>}
          {settings.storeAddress && <div style={{ whiteSpace: 'pre-line' }}>{settings.storeAddress}</div>}
          {settings.storePhone && <div>Tel: {settings.storePhone}</div>}
          {settings.storeEmail && <div>{settings.storeEmail}</div>}
          {settings.storeTaxId && <div>Tax ID: {settings.storeTaxId}</div>}
        </div>
        <div style={{ textAlign: 'right' }}>
          <div style={{ fontSize: 20, fontWeight: 700 }}>{sale.status === 'void' ? 'VOID INVOICE' : 'INVOICE'}</div>
          {reprint && <div>Duplicate copy</div>}
          <div style={{ marginTop: 6 }}>
            No. <b>{sale.invoiceNo}</b>
          </div>
          <div>{dateTime(sale.createdAt)}</div>
          <div>Cashier: {sale.cashierName}</div>
          <div style={{ marginTop: 6 }}>
            <Barcode value={sale.invoiceNo} height={30} width={1.2} fontSize={9} />
          </div>
        </div>
      </div>
      {sale.customerName && (
        <div style={{ marginTop: 14 }}>
          <div style={{ fontSize: 10, textTransform: 'uppercase', letterSpacing: 1 }}>Bill to</div>
          <div style={{ fontWeight: 700 }}>{sale.customerName}</div>
          {sale.customerPhone && <div>{sale.customerPhone}</div>}
        </div>
      )}
      <table style={{ width: '100%', borderCollapse: 'collapse', marginTop: 16 }}>
        <thead>
          <tr>
            <th style={th}>#</th>
            <th style={th}>Item</th>
            <th style={{ ...th, ...num }}>Qty</th>
            <th style={{ ...th, ...num }}>Price</th>
            <th style={{ ...th, ...num }}>Discount</th>
            <th style={{ ...th, ...num }}>Amount</th>
          </tr>
        </thead>
        <tbody>
          {sale.items.map((i) => (
            <tr key={i.id}>
              <td style={td}>{i.lineNo}</td>
              <td style={td}>
                {i.productName}
                <div style={{ fontSize: 10, color: '#555' }}>{i.sku}</div>
              </td>
              <td style={{ ...td, ...num }}>
                {qty(i.qty)} {i.unitName}
              </td>
              <td style={{ ...td, ...num }}>{m(i.unitPrice)}</td>
              <td style={{ ...td, ...num }}>{i.lineDiscount > 0 ? `-${m(i.lineDiscount)}` : ''}</td>
              <td style={{ ...td, ...num }}>{m(i.grossAmount - i.lineDiscount)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 12 }}>
        <div style={{ width: 260 }}>
          <Row left="Subtotal" right={m(sale.subtotal)} />
          {sale.discountTotal > 0 && <Row left="Discount" right={`-${m(sale.discountTotal)}`} />}
          <div style={{ borderTop: '1.5px solid #000', margin: '6px 0' }} />
          <Row left="Total" right={`${settings.currencySymbol} ${m(sale.total)}`} bold big />
          {sale.payments.map((p, idx) => (
            <Row key={idx} left={`Paid (${methodLabel(p.method)})`} right={m(p.amount)} />
          ))}
          {sale.tenderedCash > 0 && <Row left="Change" right={m(sale.changeAmount)} />}
          {sale.creditAmount > 0 && <Row left="Balance due" right={m(sale.creditAmount)} bold />}
        </div>
      </div>
      {settings.receiptFooter && <div style={{ marginTop: 28, textAlign: 'center', whiteSpace: 'pre-line' }}>{settings.receiptFooter}</div>}
    </div>
  );
}
