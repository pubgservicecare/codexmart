import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  CircleCheck,
  DatabaseBackup,
  FolderOpen,
  HardDrive,
  Clock3,
  RotateCcw,
  ShieldCheck,
  TriangleAlert,
  Usb,
} from 'lucide-react';
import { api, type BackupCheck, type Settings } from '@/lib/api';
import { bytes, dateTime, relativeTime } from '@/lib/format';
import { useSession, useSettings } from '@/lib/session';
import { cx } from '@/lib/utils';
import { Badge, Button, EmptyState, Field, Input, Modal, PageHeader, Select, Spinner, Switch, useToast } from '@/components/ui';

export function BackupPage() {
  const { can, status } = useSession();
  const toast = useToast();
  const { data, isLoading, refetch } = useQuery({ queryKey: ['backup-status'], queryFn: api.backup.status });
  const [busy, setBusy] = useState<string | null>(null);
  const [restoreFile, setRestoreFile] = useState<string | null>(null);

  const backupNow = async (dest?: string) => {
    setBusy(dest ? 'usb' : 'now');
    try {
      const r = await api.backup.create(dest);
      toast.success('Backup created and verified', `${r.fileName} (${bytes(r.sizeBytes)})`);
      void refetch();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(null);
    }
  };

  const backupToFolder = async () => {
    const dir = await open({ directory: true, title: 'Choose a USB drive or folder for the backup' });
    if (typeof dir === 'string') await backupNow(dir);
  };

  const chooseRestore = async () => {
    const file = await open({ multiple: false, title: 'Choose a CODE X MART backup', filters: [{ name: 'CODE X MART backup', extensions: ['cxmbak'] }] });
    if (typeof file === 'string') setRestoreFile(file);
  };

  if (isLoading || !data) return <Spinner />;
  const lastAgeHours = data.lastSuccessAt ? (Date.now() - new Date(data.lastSuccessAt).getTime()) / 3_600_000 : Infinity;

  return (
    <div className="space-y-5">
      <PageHeader
        title="Backup & restore"
        description="Your shop data lives only on this computer. Backups protect it from disk failure, theft and Windows problems."
        icon={<DatabaseBackup className="h-5 w-5" />}
        actions={
          <>
            {can('restoreBackup') && (
              <Button icon={<RotateCcw className="h-4 w-4" />} onClick={chooseRestore}>
                Restore from backup…
              </Button>
            )}
            <Button icon={<Usb className="h-4 w-4" />} loading={busy === 'usb'} onClick={backupToFolder}>
              Back up to USB / folder…
            </Button>
            <Button variant="primary" icon={<DatabaseBackup className="h-4 w-4" />} loading={busy === 'now'} onClick={() => void backupNow()}>
              Back up now
            </Button>
          </>
        }
      />

      {data.sameDriveOnly && (
        <div className="flex items-start gap-3 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
          <TriangleAlert className="mt-0.5 h-5 w-5 shrink-0" />
          <div>
            <p className="font-semibold">All backups are on the same drive as your data.</p>
            <p>
              If this computer's disk fails, the backups are lost too. Set a USB drive or second disk as the “copy automatic backups to” folder below,
              and regularly use “Back up to USB / folder”.
            </p>
          </div>
        </div>
      )}

      <div className="grid grid-cols-3 gap-4">
        <div className="card p-5">
          <div className="flex items-center gap-2">
            {lastAgeHours < 48 ? <CircleCheck className="h-5 w-5 text-emerald-600" /> : <TriangleAlert className="h-5 w-5 text-amber-600" />}
            <p className="text-sm font-semibold text-slate-900">Last successful backup</p>
          </div>
          <p className="mt-2 text-2xl font-semibold text-slate-900">{relativeTime(data.lastSuccessAt)}</p>
          <p className="text-xs text-slate-500">{data.lastSuccessAt ? dateTime(data.lastSuccessAt) : 'No backup has been made yet'}</p>
        </div>
        <div className="card p-5">
          <div className="flex items-center gap-2">
            <ShieldCheck className="h-5 w-5 text-brand-600" />
            <p className="text-sm font-semibold text-slate-900">Automatic backups</p>
          </div>
          <p className="mt-2 text-2xl font-semibold text-slate-900">{data.autoEnabled ? `Every ${data.intervalHours} h` : 'Off'}</p>
          <p className="text-xs text-slate-500">Keeps the latest {data.retention} automatic backups. Manual backups are never deleted.</p>
        </div>
        <div className="card p-5">
          <div className="flex items-center gap-2">
            <HardDrive className="h-5 w-5 text-slate-600" />
            <p className="text-sm font-semibold text-slate-900">Second copy (USB / other drive)</p>
          </div>
          {data.secondaryDir ? (
            <>
              <p className="mt-2 break-all text-sm font-medium text-slate-900">{data.secondaryDir}</p>
              <Badge tone={data.secondaryAvailable ? 'green' : 'red'} className="mt-1">
                {data.secondaryAvailable ? 'Connected' : 'Not connected'}
              </Badge>
            </>
          ) : (
            <p className="mt-2 text-sm text-slate-500">Not set. Choose a folder in the settings below.</p>
          )}
        </div>
      </div>

      {can('manageSettings') && <BackupSettings onSaved={() => void refetch()} />}

      <div className="grid grid-cols-2 gap-5">
        <div className="card">
          <div className="card-header">
            <h2 className="card-title">Backups in the default folder</h2>
            <Button size="sm" variant="ghost" icon={<FolderOpen className="h-4 w-4" />} onClick={() => void api.app.openFolder('backups')}>
              Open folder
            </Button>
          </div>
          <p className="px-5 pt-3 font-mono text-2xs text-slate-500">{data.backupDir}</p>
          <div className="max-h-96 overflow-y-auto">
            {data.files.length === 0 ? (
              <EmptyState title="No backup files yet" />
            ) : (
              <table className="table">
                <tbody>
                  {data.files.map((f) => (
                    <tr key={f.path}>
                      <td>
                        <p className="font-mono text-xs text-slate-800">{f.fileName}</p>
                        <p className="text-2xs text-slate-500">
                          {dateTime(f.modifiedAt)} · {bytes(f.sizeBytes)}
                        </p>
                      </td>
                      <td>
                        <Badge tone={f.kind === 'manual' ? 'brand' : f.kind === 'auto' ? 'gray' : 'amber'}>{f.kind}</Badge>
                      </td>
                      <td className="text-right">
                        {can('restoreBackup') && (
                          <Button size="sm" variant="ghost" onClick={() => setRestoreFile(f.path)}>
                            Restore…
                          </Button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
        <div className="card">
          <div className="card-header">
            <h2 className="card-title">History</h2>
            <Clock3 className="h-4 w-4 text-slate-400" />
          </div>
          <div className="max-h-[26rem] overflow-y-auto">
            {data.history.length === 0 ? (
              <EmptyState title="No backup activity yet" />
            ) : (
              <table className="table">
                <tbody>
                  {data.history.map((h, i) => (
                    <tr key={i}>
                      <td className="whitespace-nowrap text-xs text-slate-600">{dateTime(h.at)}</td>
                      <td className="text-xs">
                        <span className="font-medium capitalize text-slate-800">{h.action}</span> <span className="text-slate-500">({h.kind})</span>
                        {h.message && <p className="text-2xs text-rose-600">{h.message}</p>}
                        {h.path && <p className="break-all font-mono text-2xs text-slate-400">{h.path}</p>}
                      </td>
                      <td className="text-right">
                        <Badge tone={h.status === 'success' ? 'green' : 'red'}>{h.status}</Badge>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>

      <RecoveryGuide dataDir={status?.dataDir ?? ''} backupDir={data.backupDir} />
      {restoreFile && <RestoreDialog file={restoreFile} onClose={() => setRestoreFile(null)} />}
    </div>
  );
}

function BackupSettings({ onSaved }: { onSaved: () => void }) {
  const settings = useSettings();
  const qc = useQueryClient();
  const toast = useToast();
  const [form, setForm] = useState<Settings>(settings);
  const [busy, setBusy] = useState(false);
  useEffect(() => setForm(settings), [settings]);
  const set = (p: Partial<Settings>) => setForm((f) => ({ ...f, ...p }));
  const dirty =
    form.backupAutoEnabled !== settings.backupAutoEnabled ||
    form.backupIntervalHours !== settings.backupIntervalHours ||
    form.backupRetention !== settings.backupRetention ||
    form.backupOnExit !== settings.backupOnExit ||
    form.backupSecondaryDir !== settings.backupSecondaryDir;

  const save = async () => {
    setBusy(true);
    try {
      await api.settings.save(form);
      await qc.invalidateQueries({ queryKey: ['settings'] });
      toast.success('Backup settings saved');
      onSaved();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card p-5">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="card-title">Backup settings</h2>
        <Button variant="primary" size="sm" disabled={!dirty} loading={busy} onClick={save}>
          Save
        </Button>
      </div>
      <div className="grid grid-cols-2 gap-6">
        <div className="space-y-4">
          <Switch checked={form.backupAutoEnabled} onChange={(v) => set({ backupAutoEnabled: v })} label="Automatic backups" description="Runs in the background while CODE X MART is open." />
          <Switch checked={form.backupOnExit} onChange={(v) => set({ backupOnExit: v })} label="Back up when the app is closed" description="Skipped if a backup was made in the last 10 minutes." />
          <div className="grid grid-cols-2 gap-3">
            <Field label="Every">
              <Select value={form.backupIntervalHours} onChange={(e) => set({ backupIntervalHours: Number(e.target.value) })}>
                {[1, 2, 4, 6, 12, 24, 48].map((h) => (
                  <option key={h} value={h}>
                    {h} hour{h > 1 ? 's' : ''}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="Keep automatic backups">
              <Select value={form.backupRetention} onChange={(e) => set({ backupRetention: Number(e.target.value) })}>
                {[7, 14, 30, 60, 90, 180].map((n) => (
                  <option key={n} value={n}>
                    Latest {n}
                  </option>
                ))}
              </Select>
            </Field>
          </div>
        </div>
        <Field
          label="Also copy automatic backups to"
          hint="Pick a USB flash drive or a second disk (e.g. D:\CODEXMART-Backups). If it is not connected, the copy is skipped and shown in the history."
        >
          <div className="flex gap-2">
            <Input value={form.backupSecondaryDir} onChange={(e) => set({ backupSecondaryDir: e.target.value })} placeholder="Not set" />
            <Button
              icon={<FolderOpen className="h-4 w-4" />}
              onClick={async () => {
                const dir = await open({ directory: true, title: 'Folder for the second backup copy' });
                if (typeof dir === 'string') set({ backupSecondaryDir: dir });
              }}
            >
              Browse
            </Button>
          </div>
        </Field>
      </div>
    </div>
  );
}

function RestoreDialog({ file, onClose }: { file: string; onClose: () => void }) {
  const { refreshStatus } = useSession();
  const toast = useToast();
  const [check, setCheck] = useState<BackupCheck | null>(null);
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    api.backup
      .validate(file)
      .then((c) => alive && setCheck(c))
      .catch((e) => alive && setCheck({ valid: false, message: String(e?.message ?? e), manifest: null, fileSize: 0 }));
    return () => {
      alive = false;
    };
  }, [file]);

  const restore = async () => {
    setBusy(true);
    try {
      const r = await api.backup.restore(file, confirm);
      toast.success('Data restored', r.safetyBackup ? `Your previous data was saved first: ${r.safetyBackup}` : undefined);
      await refreshStatus();
    } catch (e) {
      toast.error(e);
      setBusy(false);
    }
  };

  const m = check?.manifest;
  return (
    <Modal
      open
      onClose={onClose}
      title="Restore from backup"
      icon={<RotateCcw className="h-5 w-5" />}
      closeOnBackdrop={false}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="danger" loading={busy} disabled={!check?.valid || confirm !== 'RESTORE'} onClick={restore}>
            Replace current data
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <p className="break-all font-mono text-xs text-slate-600">{file}</p>
        {!check ? (
          <Spinner label="Checking the backup…" />
        ) : (
          <div className={cx('rounded-lg px-4 py-3 text-sm', check.valid ? 'bg-emerald-50 text-emerald-900' : 'bg-rose-50 text-rose-800')}>
            <p className="font-semibold">{check.valid ? 'Backup verified' : 'This backup cannot be restored'}</p>
            <p>{check.message}</p>
          </div>
        )}
        {m && (
          <div className="grid grid-cols-2 gap-x-6 gap-y-1 rounded-lg bg-slate-50 p-4 text-sm">
            <span className="text-slate-500">Made</span>
            <span>{dateTime(m.createdAt)}</span>
            <span className="text-slate-500">Store</span>
            <span>{m.storeName ?? '—'}</span>
            <span className="text-slate-500">App version</span>
            <span>
              {m.appVersion} (database v{m.schemaVersion})
            </span>
            <span className="text-slate-500">Contains</span>
            <span>
              {m.counts.products ?? 0} products, {m.counts.sales ?? 0} sales, {m.counts.customers ?? 0} customers
            </span>
          </div>
        )}
        {check?.valid && (
          <>
            <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
              Everything recorded after this backup was made will be replaced. A safety backup of the current data is created automatically
              first, and everyone is signed out.
            </div>
            <Field label='Type "RESTORE" to confirm'>
              <Input value={confirm} onChange={(e) => setConfirm(e.target.value)} />
            </Field>
          </>
        )}
      </div>
    </Modal>
  );
}

function RecoveryGuide({ dataDir, backupDir }: { dataDir: string; backupDir: string }) {
  return (
    <div className="card p-6">
      <h2 className="card-title">If Windows is reinstalled or the computer is replaced</h2>
      <ol className="mt-3 list-decimal space-y-1.5 pl-5 text-sm text-slate-700">
        <li>Before reinstalling, copy your latest backup file (.cxmbak) to a USB drive. Backups are in <span className="font-mono text-xs">{backupDir}</span>.</li>
        <li>Install CODE X MART on the new or repaired Windows.</li>
        <li>On first start you will see the setup screen. Create a temporary administrator — it is replaced by the restore.</li>
        <li>Go to Backup & restore → “Restore from backup…”, choose the .cxmbak file from the USB drive and type RESTORE.</li>
        <li>Sign in with your usual username and password from the backup.</li>
      </ol>
      <p className="mt-3 text-xs text-slate-500">
        All data is kept in <span className="font-mono">{dataDir}</span>. Uninstalling or updating CODE X MART never deletes this folder.
      </p>
    </div>
  );
}
