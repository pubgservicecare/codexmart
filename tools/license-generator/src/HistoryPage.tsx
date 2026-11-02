import { useEffect, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { ArrowRightLeft, Download, History, RefreshCw } from 'lucide-react';
import { Badge, Button, EmptyState, Modal, PageHeader, SearchBox, useToast } from '@/components/ui';
import { useDebounced } from '@/lib/utils';
import { errorText, gen, type IssuedRecord } from './api';

const STATUS_TONE = { issued: 'green', superseded: 'gray', transferred: 'blue' } as const;

export function HistoryPage({ onAction }: { onAction: (r: IssuedRecord, mode: 'reissue' | 'transfer') => void }) {
  const toast = useToast();
  const [query, setQuery] = useState('');
  const [rows, setRows] = useState<IssuedRecord[]>([]);
  const [selected, setSelected] = useState<IssuedRecord | null>(null);
  const q = useDebounced(query, 200);

  useEffect(() => {
    gen.history(q).then(setRows, (e) => toast.error(errorText(e)));
  }, [q, toast]);

  return (
    <div className="space-y-5">
      <PageHeader
        title="License history"
        description="Every activation key you have issued, stored only on this computer."
        icon={<History className="h-5 w-5" />}
        actions={
          <Button
            icon={<Download className="h-4 w-4" />}
            onClick={async () => {
              const path = await save({ title: 'Export license history', defaultPath: 'codexmart-licenses.csv', filters: [{ name: 'CSV', extensions: ['csv'] }] });
              if (!path) return;
              try {
                const n = await gen.exportHistory(path);
                toast.success('History exported', `${n} licenses. The file contains customer names and phone numbers: keep it private.`);
              } catch (e) {
                toast.error(errorText(e));
              }
            }}
          >
            Export CSV
          </Button>
        }
      />
      <SearchBox value={query} onChange={setQuery} placeholder="Search name, phone, serial, device ID or notes" />
      <div className="card overflow-hidden">
        {rows.length === 0 ? (
          <EmptyState icon={<History className="h-6 w-6" />} title="No licenses found" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>License no.</th>
                <th>Customer</th>
                <th>Type</th>
                <th>Issued</th>
                <th>Valid until</th>
                <th>Computer</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.serial} className="cursor-pointer hover:bg-slate-50" onClick={() => setSelected(r)}>
                  <td className="font-mono text-xs">{r.serial}</td>
                  <td>
                    {r.customerName}
                    <span className="block text-2xs text-slate-500">{r.customerPhone}</span>
                  </td>
                  <td>{r.editionLabel}</td>
                  <td>{r.issuedOn}</td>
                  <td>{r.expiresOn ?? 'Lifetime'}</td>
                  <td className="font-mono text-xs">{r.deviceId}</td>
                  <td>
                    <Badge tone={STATUS_TONE[r.status]}>{r.status}</Badge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <Modal
        open={!!selected}
        onClose={() => setSelected(null)}
        size="lg"
        title={selected?.serial ?? ''}
        description={selected ? `${selected.editionLabel} · ${selected.customerName}` : undefined}
        footer={
          selected && (
            <>
              <Button icon={<RefreshCw className="h-4 w-4" />} onClick={() => onAction(selected, 'reissue')}>
                Renew / reissue
              </Button>
              <Button icon={<ArrowRightLeft className="h-4 w-4" />} onClick={() => onAction(selected, 'transfer')}>
                Transfer to new computer
              </Button>
            </>
          )
        }
      >
        {selected && (
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
            <dt className="text-slate-500">Customer</dt>
            <dd>
              {selected.customerName} (no. {selected.customerNo}) {selected.customerPhone}
            </dd>
            <dt className="text-slate-500">Computer</dt>
            <dd className="font-mono">{selected.deviceId}</dd>
            <dt className="text-slate-500">Valid</dt>
            <dd>
              {selected.issuedOn} – {selected.expiresOn ?? 'no expiry'}
            </dd>
            <dt className="text-slate-500">Kind</dt>
            <dd>
              {selected.kind}
              {selected.replaces ? ` (replaces ${selected.replaces})` : ''} · {selected.status}
            </dd>
            <dt className="text-slate-500">Signed with key</dt>
            <dd className="font-mono">{selected.keyFingerprint}</dd>
            <dt className="text-slate-500">Notes</dt>
            <dd>{selected.notes || '—'}</dd>
            <dt className="text-slate-500">Request code</dt>
            <dd className="break-all font-mono text-xs">{selected.requestCode}</dd>
            <dt className="text-slate-500">Activation key</dt>
            <dd className="select-all break-all font-mono text-xs">{selected.activationKey}</dd>
          </dl>
        )}
      </Modal>
    </div>
  );
}
