import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { DatabaseBackup, LockKeyhole, LogOut, ShieldCheck, TriangleAlert, Usb } from 'lucide-react';
import { api } from '@/lib/api';
import { bytes, date } from '@/lib/format';
import { useSession } from '@/lib/session';
import { Button, useToast } from '@/components/ui';
import { ActivationForm, LicenseFacts, OwnerContactCard, RequestCodePanel } from './LicenseParts';

/**
 * Shown after sign-in when the free trial or license has ended. Business
 * screens stay closed until a valid key is activated; shop data is never
 * touched and can still be backed up from here.
 */
export function LicenseScreen() {
  const { license, setLicense, session, settings, signOut, can } = useSession();
  const toast = useToast();
  const [busy, setBusy] = useState<string | null>(null);
  if (!license) return null;

  const backup = async (dest?: string) => {
    setBusy(dest ? 'usb' : 'now');
    try {
      const r = await api.backup.create(dest);
      toast.success('Backup created and verified', `${r.fileName} (${bytes(r.sizeBytes)})`);
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex h-full overflow-hidden bg-ink-950">
      <aside className="relative hidden w-[40%] max-w-xl flex-col overflow-y-auto p-10 lg:flex">
        <div className="absolute -left-32 -top-32 h-96 w-96 rounded-full bg-brand-600/30 blur-3xl" />
        <div className="absolute -bottom-40 right-0 h-[28rem] w-[28rem] rounded-full bg-violet-700/20 blur-3xl" />
        <div className="relative flex items-center gap-3">
          <img src="/logo.svg" alt="" className="h-11 w-11" />
          <div className="leading-tight">
            <p className="text-lg font-bold tracking-tight text-white">CODE X MART</p>
            <p className="text-xs text-slate-400">{settings?.storeName}</p>
          </div>
        </div>
        <div className="relative mt-auto pt-16">
          <div className="mb-5 flex h-12 w-12 items-center justify-center rounded-2xl bg-white/10 text-amber-300">
            <LockKeyhole className="h-6 w-6" />
          </div>
          <h1 className="text-3xl font-semibold leading-tight tracking-tight text-white">{license.headline}</h1>
          <p className="mt-3 text-base text-slate-300">Contact the software owner to activate your license.</p>
          <p className="mt-2 text-sm text-slate-400">{license.message}</p>
          {license.trialStartedOn && license.trialLastDay && (
            <p className="mt-5 text-sm text-slate-400">
              Free trial: {date(license.trialStartedOn)} – {date(license.trialLastDay)}
            </p>
          )}
          <OwnerContactCard owner={license.owner} className="mt-8" />
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto bg-[var(--canvas)] lg:rounded-l-[28px]">
        <div className="mx-auto max-w-2xl space-y-5 px-6 py-10">
          <div className="lg:hidden">
            <h1 className="text-2xl font-semibold text-slate-900">{license.headline}</h1>
            <p className="mt-2 text-sm text-slate-600">{license.message}</p>
            <OwnerContactCard owner={license.owner} className="mt-5" />
          </div>

          {license.clockWarning && (
            <div className="flex gap-3 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
              <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
              {license.clockWarning}
            </div>
          )}

          <section className="card p-6">
            <p className="text-2xs font-semibold uppercase tracking-wider text-brand-600">Step 1</p>
            <h2 className="mt-1 text-lg font-semibold text-slate-900">Request your activation code</h2>
            <p className="mb-4 mt-1 text-sm text-slate-500">
              Send this computer's request code to {license.owner.name}. You will receive an activation key in return.
            </p>
            <RequestCodePanel />
          </section>

          <section className="card p-6">
            <p className="text-2xs font-semibold uppercase tracking-wider text-brand-600">Step 2</p>
            <h2 className="mt-1 text-lg font-semibold text-slate-900">Enter the activation key</h2>
            <p className="mb-4 mt-1 text-sm text-slate-500">The key is checked on this computer. No internet connection is needed.</p>
            <ActivationForm onActivated={setLicense} />
          </section>

          <section className="card p-6">
            <div className="flex items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-emerald-50 text-emerald-600">
                <ShieldCheck className="h-5 w-5" />
              </div>
              <div className="min-w-0 flex-1">
                <h2 className="font-semibold text-slate-900">Your shop data is safe</h2>
                <p className="mt-1 text-sm text-slate-500">
                  Nothing has been deleted or changed. Products, sales, customers and reports are all kept and open again as soon
                  as the license is activated.
                </p>
                {can('createBackup') && (
                  <div className="mt-4 flex flex-wrap gap-2">
                    <Button icon={<DatabaseBackup className="h-4 w-4" />} loading={busy === 'now'} onClick={() => backup()}>
                      Back up now
                    </Button>
                    <Button
                      icon={<Usb className="h-4 w-4" />}
                      loading={busy === 'usb'}
                      onClick={async () => {
                        const dir = await open({ directory: true, title: 'Choose a USB drive or folder for the backup' });
                        if (typeof dir === 'string') await backup(dir);
                      }}
                    >
                      Back up to USB / folder…
                    </Button>
                  </div>
                )}
                <div className="mt-5 border-t border-slate-100 pt-4">
                  <LicenseFacts license={license} />
                </div>
              </div>
            </div>
          </section>

          <div className="flex items-center justify-between text-sm text-slate-500">
            <span>
              Signed in as <span className="font-medium text-slate-700">{session?.user.fullName}</span>
            </span>
            <Button variant="ghost" size="sm" icon={<LogOut className="h-4 w-4" />} onClick={signOut}>
              Sign out
            </Button>
          </div>
        </div>
      </main>
    </div>
  );
}
