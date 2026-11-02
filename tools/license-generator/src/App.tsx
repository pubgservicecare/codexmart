import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { FileKey2, History, KeyRound, Lock, ShieldAlert, ShieldCheck, Stamp, TriangleAlert } from 'lucide-react';
import { Button, Field, Input, Spinner, useToast } from '@/components/ui';
import { cx } from '@/lib/utils';
import { errorText, gen, type GenStatus, type IssuedRecord } from './api';
import { IssuePage } from './IssuePage';
import { HistoryPage } from './HistoryPage';
import { KeyPage } from './KeyPage';

type Page = 'issue' | 'history' | 'key';

function Centered({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center overflow-y-auto bg-ink-950 p-6">
      <div className="w-full max-w-3xl">
        <div className="mb-6 flex items-center gap-3 text-white">
          <img src="/logo.svg" alt="" className="h-10 w-10" />
          <div>
            <p className="text-lg font-bold tracking-tight">CODE X MART License Generator</p>
            <p className="text-xs text-slate-400">Owner only · keep this program and its key backup private</p>
          </div>
        </div>
        {children}
      </div>
    </div>
  );
}

function PassphraseFields({ onSubmit, label, busy }: { onSubmit: (p: string, c: string) => void; label: string; busy: boolean }) {
  const [p, setP] = useState('');
  const [c, setC] = useState('');
  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit(p, c);
      }}
    >
      <Field label="Owner passphrase" hint="At least 10 characters. A short sentence is easy to remember and hard to guess.">
        <Input type="password" value={p} onChange={(e) => setP(e.target.value)} autoFocus />
      </Field>
      <Field label="Repeat passphrase">
        <Input type="password" value={c} onChange={(e) => setC(e.target.value)} />
      </Field>
      <Button type="submit" variant="primary" loading={busy} disabled={!p || !c}>
        {label}
      </Button>
    </form>
  );
}

function SetupScreen({ onDone }: { onDone: () => void }) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const [file, setFile] = useState<string | null>(null);
  const [restorePass, setRestorePass] = useState('');
  const run = async (f: () => Promise<unknown>, msg: string) => {
    setBusy(true);
    try {
      await f();
      toast.success(msg);
      onDone();
    } catch (e) {
      toast.error(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Centered>
      <div className="grid gap-5 md:grid-cols-2">
        <div className="card p-6">
          <KeyRound className="h-6 w-6 text-brand-600" />
          <h2 className="mt-3 text-lg font-semibold text-slate-900">Create a new signing key</h2>
          <p className="mb-4 mt-1 text-sm text-slate-500">
            For first-time setup. The key's public half must then be built into CODE X MART (see the Owner Key Management guide).
          </p>
          <PassphraseFields busy={busy} label="Create signing key" onSubmit={(p, c) => run(() => gen.createKey(p, c), 'Signing key created')} />
        </div>
        <div className="card p-6">
          <FileKey2 className="h-6 w-6 text-brand-600" />
          <h2 className="mt-3 text-lg font-semibold text-slate-900">Restore from a key backup</h2>
          <p className="mb-4 mt-1 text-sm text-slate-500">On a new or repaired computer: choose your .cxkey backup file and enter its passphrase.</p>
          <Button
            onClick={async () => {
              const f = await open({ title: 'Choose your signing key backup', filters: [{ name: 'Signing key backup', extensions: ['cxkey'] }] });
              if (typeof f === 'string') setFile(f);
            }}
          >
            {file ? 'Change file' : 'Choose backup file…'}
          </Button>
          {file && <p className="mt-2 break-all font-mono text-xs text-slate-600">{file}</p>}
          <Field label="Backup passphrase" className="mt-3">
            <Input type="password" value={restorePass} onChange={(e) => setRestorePass(e.target.value)} />
          </Field>
          <Button
            className="mt-3"
            variant="primary"
            loading={busy}
            disabled={!file || !restorePass}
            onClick={() => run(() => gen.restoreKey(file!, restorePass), 'Signing key restored')}
          >
            Restore signing key
          </Button>
        </div>
      </div>
    </Centered>
  );
}

function PendingScreen({ onDone }: { onDone: () => void }) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  return (
    <Centered>
      <div className="card p-6">
        <ShieldAlert className="h-6 w-6 text-amber-600" />
        <h2 className="mt-3 text-lg font-semibold text-slate-900">Choose your owner passphrase</h2>
        <p className="mb-4 mt-1 text-sm text-slate-600">
          Your signing key was created during installation of this release and is protected by your Windows account only. Choose a
          passphrase now: it will be needed every time you issue licenses, and it protects your key backup. After this, save a key
          backup on a USB drive.
        </p>
        <PassphraseFields
          busy={busy}
          label="Protect signing key"
          onSubmit={async (p, c) => {
            setBusy(true);
            try {
              await gen.setPassphrase(p, c);
              toast.success('Passphrase set', 'Now save a backup of your signing key.');
              onDone();
            } catch (e) {
              toast.error(errorText(e));
            } finally {
              setBusy(false);
            }
          }}
        />
      </div>
    </Centered>
  );
}

function UnlockScreen({ onDone }: { onDone: () => void }) {
  const [p, setP] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <Centered>
      <form
        className="card mx-auto max-w-md p-6"
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError(null);
          try {
            await gen.unlock(p);
            onDone();
          } catch (err) {
            setError(errorText(err));
          } finally {
            setBusy(false);
          }
        }}
      >
        <Lock className="h-6 w-6 text-brand-600" />
        <h2 className="mt-3 text-lg font-semibold text-slate-900">Unlock</h2>
        <p className="mb-4 mt-1 text-sm text-slate-500">Enter your owner passphrase to issue licenses.</p>
        <Field label="Owner passphrase" error={error}>
          <Input type="password" value={p} onChange={(e) => setP(e.target.value)} autoFocus />
        </Field>
        <Button type="submit" variant="primary" className="mt-4" loading={busy} disabled={!p}>
          Unlock
        </Button>
      </form>
    </Centered>
  );
}

export function App() {
  const [status, setStatus] = useState<GenStatus | null>(null);
  const [page, setPage] = useState<Page>('issue');
  const [prefill, setPrefill] = useState<{ record: IssuedRecord; mode: 'reissue' | 'transfer' } | null>(null);
  const refresh = useCallback(async () => setStatus(await gen.status()), []);

  useEffect(() => {
    void refresh();
    // Re-check now and then so an idle lock shows up.
    const id = setInterval(() => void refresh(), 60_000);
    return () => clearInterval(id);
  }, [refresh]);

  if (!status) return <Spinner label="Starting…" />;
  if (status.keyState === 'none') return <SetupScreen onDone={refresh} />;
  if (status.keyState === 'pending') return <PendingScreen onDone={refresh} />;
  if (status.keyState === 'locked') return <UnlockScreen onDone={refresh} />;

  const key = status.key!;
  const nav: { key: Page; label: string; icon: ReactNode }[] = [
    { key: 'issue', label: 'Issue license', icon: <Stamp /> },
    { key: 'history', label: 'License history', icon: <History /> },
    { key: 'key', label: 'Signing key', icon: <KeyRound /> },
  ];
  return (
    <div className="flex h-full">
      <aside className="flex w-60 shrink-0 flex-col bg-ink-950 text-slate-300">
        <div className="flex items-center gap-2.5 px-5 pb-4 pt-4">
          <img src="/logo.svg" alt="" className="h-9 w-9" />
          <div className="leading-tight">
            <p className="text-[15px] font-bold tracking-tight text-white">License Generator</p>
            <p className="text-2xs text-slate-400">CODE X MART · owner only</p>
          </div>
        </div>
        <nav className="flex-1 space-y-0.5 px-3">
          {nav.map((n) => (
            <button
              key={n.key}
              onClick={() => setPage(n.key)}
              className={cx(
                'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13px] font-medium transition [&>svg]:h-[18px] [&>svg]:w-[18px]',
                page === n.key ? 'bg-white/10 text-white' : 'text-slate-400 hover:bg-white/5 hover:text-slate-100',
              )}
            >
              {n.icon}
              {n.label}
            </button>
          ))}
        </nav>
        <div className="border-t border-white/5 p-3">
          <p className="px-2 text-2xs text-slate-500">Key {key.fingerprint}</p>
          <Button
            variant="ghost"
            size="sm"
            className="mt-2 w-full !justify-start !text-slate-300 hover:!bg-white/5"
            icon={<Lock className="h-4 w-4" />}
            onClick={async () => {
              await gen.lock();
              await refresh();
            }}
          >
            Lock
          </Button>
        </div>
      </aside>
      <main className="min-w-0 flex-1 overflow-y-auto px-6 py-6">
        {!key.lastBackupAt && (
          <div className="mb-5 flex items-start gap-3 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
            <span>
              Save a backup of your signing key before issuing licenses.{' '}
              <button className="font-semibold underline" onClick={() => setPage('key')}>
                Open Signing key
              </button>
            </span>
          </div>
        )}
        {!key.trustedByApp && (
          <div className="mb-5 flex items-start gap-3 rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-900">
            <ShieldAlert className="mt-0.5 h-4 w-4 shrink-0" />
            This signing key is not built into this version of CODE X MART, so its activation keys will be refused. See the Owner Key
            Management guide (add the public key and rebuild the installer).
          </div>
        )}
        {key.trustedByApp && page === 'key' && (
          <p className="mb-3 flex items-center gap-2 text-sm text-emerald-700">
            <ShieldCheck className="h-4 w-4" /> Trusted by CODE X MART {status.version}
          </p>
        )}
        {page === 'issue' && <IssuePage prefill={prefill} onIssued={refresh} clearPrefill={() => setPrefill(null)} />}
        {page === 'history' && (
          <HistoryPage
            onAction={(record, mode) => {
              setPrefill({ record, mode });
              setPage('issue');
            }}
          />
        )}
        {page === 'key' && <KeyPage status={status} onChange={refresh} />}
      </main>
    </div>
  );
}
