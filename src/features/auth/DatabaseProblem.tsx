import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { DatabaseBackup, FolderOpen, TriangleAlert } from 'lucide-react';
import { api, errorMessage, type BackupCheck } from '@/lib/api';
import { useSession } from '@/lib/session';
import { Button, Field, Input } from '@/components/ui';
import { date } from '@/lib/format';
import { AuthLayout } from './AuthLayout';

/**
 * Shown when the database cannot be opened. Nothing is ever reset
 * automatically: the owner can restore a backup or contact support.
 */
export function DatabaseProblem({ message }: { message: string }) {
  const { status, refreshStatus } = useSession();
  const [file, setFile] = useState<string | null>(null);
  const [check, setCheck] = useState<BackupCheck | null>(null);
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const choose = async () => {
    const picked = await open({ multiple: false, filters: [{ name: 'CODE X MART backup', extensions: ['cxmbak'] }] });
    if (typeof picked !== 'string') return;
    setFile(picked);
    setCheck(null);
    setError(null);
    try {
      setCheck(await api.backup.validate(picked));
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const retry = async () => {
    setError(null);
    try {
      await api.app.retryOpen();
    } catch (e) {
      setError(`Still cannot open the data: ${errorMessage(e)}`);
    }
    await refreshStatus();
  };

  const restore = async () => {
    if (!file) return;
    setBusy(true);
    try {
      await api.backup.restore(file, confirm);
      await refreshStatus();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthLayout wide>
      <div className="card p-7">
        <div className="flex items-start gap-3">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-rose-50 text-rose-600">
            <TriangleAlert className="h-5 w-5" />
          </div>
          <div>
            <h2 className="text-lg font-semibold text-slate-900">The shop data could not be opened</h2>
            <p className="mt-1 text-sm text-slate-600">{message}</p>
            <p className="mt-2 text-sm text-slate-600">
              Your data file has <strong>not</strong> been changed or deleted. You can restore your most recent backup below.
            </p>
          </div>
        </div>
        <div className="mt-6 rounded-xl bg-slate-50 p-4 text-xs text-slate-500">
          <p>Data file: <span className="font-mono text-slate-700">{status?.dbPath}</span></p>
          <p className="mt-1">Automatic backups: <span className="font-mono text-slate-700">{status?.backupDir}</span></p>
        </div>
        <div className="mt-6 space-y-4">
          <Button icon={<FolderOpen className="h-4 w-4" />} onClick={choose}>
            Choose a backup file (.cxmbak)
          </Button>
          {file && <p className="break-all font-mono text-xs text-slate-600">{file}</p>}
          {check && (
            <div className={`rounded-lg px-3 py-2 text-sm ${check.valid ? 'bg-emerald-50 text-emerald-800' : 'bg-rose-50 text-rose-700'}`}>
              {check.message}
              {check.manifest && (
                <p className="mt-1 text-xs">
                  Made {date(check.manifest.createdAt)} · {check.manifest.counts.sales ?? 0} sales · {check.manifest.counts.products ?? 0} products
                </p>
              )}
            </div>
          )}
          {check?.valid && (
            <Field label='Type "RESTORE" to replace the damaged data with this backup'>
              <Input value={confirm} onChange={(e) => setConfirm(e.target.value)} />
            </Field>
          )}
          {error && <p className="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700">{error}</p>}
          <div className="flex gap-2">
            <Button variant="primary" icon={<DatabaseBackup className="h-4 w-4" />} disabled={!check?.valid || confirm !== 'RESTORE'} loading={busy} onClick={restore}>
              Restore backup
            </Button>
            <Button variant="ghost" onClick={() => void retry()}>
              Try opening again
            </Button>
          </div>
        </div>
      </div>
    </AuthLayout>
  );
}
