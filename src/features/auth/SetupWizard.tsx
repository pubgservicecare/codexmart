import { useState } from 'react';
import { ArrowRight, Check, Copy, KeyRound, Store } from 'lucide-react';
import { api, errorMessage, type Session } from '@/lib/api';
import { useSession } from '@/lib/session';
import { Button, Field, Input, Textarea } from '@/components/ui';
import { AuthLayout } from './AuthLayout';

/** First run: store details and the administrator account. */
export function SetupWizard() {
  const { setSession, refreshStatus } = useSession();
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [store, setStore] = useState({ name: '', phone: '', address: '', currency: 'Rs' });
  const [admin, setAdmin] = useState({ fullName: '', username: 'admin', password: '', confirm: '' });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ session: Session; recoveryKey: string } | null>(null);
  const [copied, setCopied] = useState(false);
  const [acknowledged, setAcknowledged] = useState(false);

  const create = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.app.setup({
        storeName: store.name,
        storePhone: store.phone,
        storeAddress: store.address,
        currencySymbol: store.currency,
        adminFullName: admin.fullName,
        adminUsername: admin.username,
        adminPassword: admin.password,
      });
      setResult(r);
      setStep(3);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const finish = async () => {
    if (!result) return;
    await refreshStatus();
    setSession(result.session);
  };

  return (
    <AuthLayout wide>
      <div className="mb-6 flex items-center gap-2 text-xs font-medium text-slate-500">
        {['Your store', 'Administrator', 'Recovery key'].map((label, i) => (
          <div key={label} className="flex items-center gap-2">
            <span
              className={`flex h-6 w-6 items-center justify-center rounded-full text-2xs font-bold ${
                step > i + 1 ? 'bg-emerald-500 text-white' : step === i + 1 ? 'bg-brand-600 text-white' : 'bg-slate-200 text-slate-500'
              }`}
            >
              {step > i + 1 ? <Check className="h-3.5 w-3.5" /> : i + 1}
            </span>
            <span className={step === i + 1 ? 'text-slate-900' : ''}>{label}</span>
            {i < 2 && <span className="mx-1 h-px w-6 bg-slate-300" />}
          </div>
        ))}
      </div>

      {step === 1 && (
        <div className="card p-7">
          <div className="mb-6 flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-brand-50 text-brand-600">
              <Store className="h-5 w-5" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-slate-900">Welcome to CODE X MART</h2>
              <p className="text-sm text-slate-500">Tell us about your store. This appears on receipts.</p>
            </div>
          </div>
          <div className="space-y-4">
            <Field label="Store name" required>
              <Input autoFocus value={store.name} onChange={(e) => setStore({ ...store, name: e.target.value })} placeholder="e.g. Al-Noor Mart" />
            </Field>
            <div className="grid grid-cols-3 gap-3">
              <Field label="Phone" className="col-span-2">
                <Input value={store.phone} onChange={(e) => setStore({ ...store, phone: e.target.value })} />
              </Field>
              <Field label="Currency symbol">
                <Input value={store.currency} onChange={(e) => setStore({ ...store, currency: e.target.value })} />
              </Field>
            </div>
            <Field label="Address">
              <Textarea rows={2} value={store.address} onChange={(e) => setStore({ ...store, address: e.target.value })} />
            </Field>
          </div>
          <div className="mt-6 flex justify-end">
            <Button variant="primary" disabled={!store.name.trim()} onClick={() => setStep(2)} icon={<ArrowRight className="h-4 w-4" />}>
              Continue
            </Button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="card p-7">
          <h2 className="text-lg font-semibold text-slate-900">Create the administrator account</h2>
          <p className="mt-1 text-sm text-slate-500">The administrator manages users, settings and backups. Add cashiers later in Settings.</p>
          <div className="mt-6 space-y-4">
            <Field label="Full name" required>
              <Input autoFocus value={admin.fullName} onChange={(e) => setAdmin({ ...admin, fullName: e.target.value })} />
            </Field>
            <Field label="Username" required hint="Letters, numbers, dot, dash or underscore.">
              <Input value={admin.username} onChange={(e) => setAdmin({ ...admin, username: e.target.value.toLowerCase() })} />
            </Field>
            <div className="grid grid-cols-2 gap-3">
              <Field label="Password" required hint="At least 4 characters.">
                <Input type="password" value={admin.password} onChange={(e) => setAdmin({ ...admin, password: e.target.value })} />
              </Field>
              <Field label="Repeat password" required error={admin.confirm && admin.confirm !== admin.password ? 'Passwords do not match' : null}>
                <Input type="password" value={admin.confirm} onChange={(e) => setAdmin({ ...admin, confirm: e.target.value })} />
              </Field>
            </div>
            {error && <p className="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700">{error}</p>}
          </div>
          <div className="mt-6 flex justify-between">
            <Button variant="ghost" onClick={() => setStep(1)}>
              Back
            </Button>
            <Button
              variant="primary"
              loading={busy}
              disabled={!admin.fullName.trim() || admin.username.length < 3 || admin.password.length < 4 || admin.password !== admin.confirm}
              onClick={create}
            >
              Create account
            </Button>
          </div>
        </div>
      )}

      {step === 3 && result && (
        <div className="card p-7">
          <div className="mb-5 flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-50 text-amber-600">
              <KeyRound className="h-5 w-5" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-slate-900">Save your recovery key</h2>
              <p className="text-sm text-slate-500">It is shown only once. Without it, a forgotten admin password cannot be reset.</p>
            </div>
          </div>
          <div className="flex items-center justify-between gap-3 rounded-xl border-2 border-dashed border-amber-300 bg-amber-50/60 px-5 py-4">
            <code className="select-all font-mono text-xl font-semibold tracking-widest text-slate-900">{result.recoveryKey}</code>
            <Button
              size="sm"
              icon={copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
              onClick={async () => {
                await navigator.clipboard.writeText(result.recoveryKey).catch(() => undefined);
                setCopied(true);
              }}
            >
              {copied ? 'Copied' : 'Copy'}
            </Button>
          </div>
          <ul className="mt-4 list-disc space-y-1 pl-5 text-sm text-slate-600">
            <li>Write it on paper and keep it with the shop documents.</li>
            <li>Do not store it only on this computer.</li>
          </ul>
          <label className="mt-5 flex cursor-pointer items-center gap-2 text-sm font-medium text-slate-800">
            <input type="checkbox" className="h-4 w-4 accent-brand-600" checked={acknowledged} onChange={(e) => setAcknowledged(e.target.checked)} />
            I have written down the recovery key
          </label>
          <div className="mt-6 flex justify-end">
            <Button variant="primary" disabled={!acknowledged} onClick={finish} icon={<ArrowRight className="h-4 w-4" />}>
              Start using CODE X MART
            </Button>
          </div>
        </div>
      )}
    </AuthLayout>
  );
}
