import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { api, type LookupKind } from '@/lib/api';
import { Button, Field, Input, Modal, Select, useToast } from '@/components/ui';

const LABELS: Record<LookupKind, string> = { category: 'category', brand: 'brand', expenseCategory: 'expense category' };

/** Select for categories / brands with an inline "add new" option. */
export function LookupSelect({
  kind,
  value,
  onChange,
  allowEmpty = true,
  canAdd = true,
  emptyLabel = 'None',
}: {
  kind: LookupKind;
  value: string | null;
  onChange: (id: string | null) => void;
  allowEmpty?: boolean;
  canAdd?: boolean;
  emptyLabel?: string;
}) {
  const toast = useToast();
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ['lookups', kind], queryFn: () => api.lookups.list(kind) });
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);

  const create = async () => {
    setBusy(true);
    try {
      const item = await api.lookups.create(kind, name);
      await qc.invalidateQueries({ queryKey: ['lookups', kind] });
      onChange(item.id);
      setAdding(false);
      setName('');
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Select
        value={value ?? ''}
        onChange={(e) => {
          if (e.target.value === '__add__') setAdding(true);
          else onChange(e.target.value || null);
        }}
      >
        {allowEmpty && <option value="">{emptyLabel}</option>}
        {data?.map((item) => (
          <option key={item.id} value={item.id}>
            {item.name}
          </option>
        ))}
        {canAdd && <option value="__add__">+ Add new {LABELS[kind]}…</option>}
      </Select>
      <Modal
        open={adding}
        onClose={() => setAdding(false)}
        size="sm"
        title={`New ${LABELS[kind]}`}
        footer={
          <>
            <Button onClick={() => setAdding(false)}>Cancel</Button>
            <Button variant="primary" loading={busy} disabled={!name.trim()} onClick={create}>
              Add
            </Button>
          </>
        }
      >
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (name.trim()) void create();
          }}
        >
          <Field label="Name">
            <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
        </form>
      </Modal>
    </>
  );
}
