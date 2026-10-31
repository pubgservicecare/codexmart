import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { UserPlus, Users } from 'lucide-react';
import { api, type CustomerBrief } from '@/lib/api';
import { money } from '@/lib/format';
import { useDebounced } from '@/lib/utils';
import { Button, EmptyState, Field, Input, Modal, SearchBox, Spinner, useToast } from '@/components/ui';

export function CustomerPicker({
  open,
  onClose,
  onPick,
}: {
  open: boolean;
  onClose: () => void;
  onPick: (c: CustomerBrief) => void;
}) {
  const [search, setSearch] = useState('');
  const [adding, setAdding] = useState(false);
  const q = useDebounced(search, 150);
  const { data, isLoading } = useQuery({
    queryKey: ['customers', 'picker', q],
    queryFn: () => api.customers.list({ search: q, limit: 30 }),
    enabled: open,
  });

  const pick = (c: CustomerBrief) => {
    onPick(c);
    setSearch('');
    onClose();
  };

  return (
    <Modal open={open} onClose={onClose} title="Select customer" icon={<Users className="h-5 w-5" />} size="md">
      {adding ? (
        <QuickAddCustomer initialName={search} onCancel={() => setAdding(false)} onCreated={(c) => pick(c)} />
      ) : (
        <div className="space-y-3">
          <div className="flex gap-2">
            <SearchBox value={search} onChange={setSearch} placeholder="Name or phone…" className="flex-1" autoFocus />
            <Button icon={<UserPlus className="h-4 w-4" />} onClick={() => setAdding(true)}>
              New
            </Button>
          </div>
          <div className="max-h-80 overflow-y-auto rounded-lg border border-slate-200">
            {isLoading ? (
              <Spinner />
            ) : !data?.items.length ? (
              <EmptyState title="No customers found" description="Add the customer to keep their purchase history and balance." />
            ) : (
              data.items.map((c) => (
                <button
                  key={c.id}
                  className="flex w-full items-center justify-between border-b border-slate-100 px-4 py-2.5 text-left last:border-0 hover:bg-brand-50/60"
                  onClick={() => pick({ id: c.id, name: c.name, phone: c.phone, balance: c.balance, creditLimit: c.creditLimit })}
                >
                  <span>
                    <span className="block text-sm font-medium text-slate-900">{c.name}</span>
                    <span className="text-xs text-slate-500">{c.phone ?? 'No phone'}</span>
                  </span>
                  {c.balance !== 0 && (
                    <span className={`num text-sm font-medium ${c.balance > 0 ? 'text-amber-700' : 'text-emerald-700'}`}>
                      {c.balance > 0 ? 'Owes ' : 'Credit '}
                      {money(Math.abs(c.balance))}
                    </span>
                  )}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}

function QuickAddCustomer({
  initialName,
  onCancel,
  onCreated,
}: {
  initialName: string;
  onCancel: () => void;
  onCreated: (c: CustomerBrief) => void;
}) {
  const toast = useToast();
  const qc = useQueryClient();
  const looksLikePhone = /^[\d+\-\s]{5,}$/.test(initialName);
  const [name, setName] = useState(looksLikePhone ? '' : initialName);
  const [phone, setPhone] = useState(looksLikePhone ? initialName : '');
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      const c = await api.customers.create({ name, phone: phone || null, openingBalance: 0 });
      void qc.invalidateQueries({ queryKey: ['customers'] });
      toast.success('Customer added', c.name);
      onCreated({ id: c.id, name: c.name, phone: c.phone, balance: c.balance, creditLimit: c.creditLimit });
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="space-y-4"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim()) void save();
      }}
    >
      <Field label="Customer name" required>
        <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} />
      </Field>
      <Field label="Phone">
        <Input value={phone} onChange={(e) => setPhone(e.target.value)} />
      </Field>
      <div className="flex justify-end gap-2">
        <Button onClick={onCancel}>Back</Button>
        <Button type="submit" variant="primary" loading={busy} disabled={!name.trim()}>
          Add customer
        </Button>
      </div>
    </form>
  );
}
