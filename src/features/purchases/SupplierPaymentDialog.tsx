import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Banknote } from 'lucide-react';
import { api, type PaymentMethod } from '@/lib/api';
import { money, today } from '@/lib/format';
import { newTxnId } from '@/lib/utils';
import { Button, Field, Input, Modal, MoneyInput, Select, useToast } from '@/components/ui';

export function SupplierPaymentDialog({
  supplierId: initialSupplier,
  purchaseId,
  onClose,
}: {
  supplierId?: string;
  purchaseId?: string;
  onClose: () => void;
}) {
  const toast = useToast();
  const qc = useQueryClient();
  const [txnId] = useState(newTxnId);
  const [supplierId, setSupplierId] = useState(initialSupplier ?? '');
  const [amount, setAmount] = useState<number | null>(null);
  const [method, setMethod] = useState<PaymentMethod>('cash');
  const [reference, setReference] = useState('');
  const [note, setNote] = useState('');
  const [day, setDay] = useState(today());
  const [busy, setBusy] = useState(false);
  const { data: suppliers } = useQuery({ queryKey: ['suppliers', 'all'], queryFn: () => api.suppliers.list({ limit: 1000 }) });
  const supplier = suppliers?.items.find((s) => s.id === supplierId);

  const save = async () => {
    if (!amount) return;
    setBusy(true);
    try {
      const p = await api.purchases.pay({
        clientTxnId: txnId,
        supplierId,
        purchaseId: purchaseId ?? null,
        amount,
        method,
        reference: reference || null,
        note: note || null,
        businessDate: day,
      });
      void qc.invalidateQueries({ queryKey: ['suppliers'] });
      void qc.invalidateQueries({ queryKey: ['purchases'] });
      void qc.invalidateQueries({ queryKey: ['dashboard'] });
      toast.success(`Payment ${p.paymentNo} recorded`, `${money(p.amount)} to ${p.supplierName}`);
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      size="sm"
      title="Pay supplier"
      description="Settles what you owe. Supplier payments are not expenses — the goods were already recorded as a purchase."
      icon={<Banknote className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!supplierId || !amount || amount <= 0} onClick={save}>
            Record payment
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field label="Supplier" required hint={supplier ? `Currently owed: ${money(supplier.balance)}` : undefined}>
          <Select value={supplierId} onChange={(e) => setSupplierId(e.target.value)} disabled={!!initialSupplier}>
            <option value="">Choose supplier…</option>
            {suppliers?.items.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </Select>
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Amount" required>
            <MoneyInput value={amount} onChange={setAmount} />
          </Field>
          <Field label="Date">
            <Input type="date" value={day} max={today()} onChange={(e) => setDay(e.target.value)} />
          </Field>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Method">
            <Select value={method} onChange={(e) => setMethod(e.target.value as PaymentMethod)}>
              <option value="cash">Cash</option>
              <option value="bank">Bank transfer</option>
              <option value="cheque">Cheque</option>
              <option value="wallet">Mobile wallet</option>
              <option value="card">Card</option>
            </Select>
          </Field>
          <Field label="Reference">
            <Input value={reference} onChange={(e) => setReference(e.target.value)} placeholder="Cheque / txn no." />
          </Field>
        </div>
        <Field label="Note">
          <Input value={note} onChange={(e) => setNote(e.target.value)} />
        </Field>
      </div>
    </Modal>
  );
}
