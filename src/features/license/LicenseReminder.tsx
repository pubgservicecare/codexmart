import { useEffect, useState } from 'react';
import { BadgeCheck, CalendarClock, KeyRound, TriangleAlert } from 'lucide-react';
import type { LicenseStatus } from '@/lib/api';
import { useSession } from '@/lib/session';
import { readStorage, writeStorage } from '@/lib/utils';
import { Badge, Button, Modal } from '@/components/ui';
import { ActivationForm, LicenseFacts, OwnerContactCard, RequestCodePanel } from './LicenseParts';

const REMINDER_KEY = 'cxm.licenseReminderShownOn';

/** Status, request code and activation in one panel (Settings → License, dialogs). */
export function LicensePanel({ onActivated }: { onActivated?: () => void }) {
  const { license, setLicense } = useSession();
  if (!license) return null;
  const tone = license.state === 'licensed' ? (license.reminder ? 'amber' : 'green') : license.allowed ? 'brand' : 'red';
  return (
    <div className="grid gap-5 lg:grid-cols-[1fr_22rem]">
      <div className="space-y-5">
        <section className="card p-6">
          <div className="flex items-start justify-between gap-4">
            <div>
              <Badge tone={tone}>
                {license.state === 'licensed' ? <BadgeCheck className="h-3.5 w-3.5" /> : <CalendarClock className="h-3.5 w-3.5" />}
                {license.state === 'licensed' ? 'Licensed' : license.state === 'notStarted' || license.state === 'trial' ? 'Free trial' : 'Activation required'}
              </Badge>
              <h2 className="mt-2 text-lg font-semibold text-slate-900">{license.headline}</h2>
              <p className="mt-1 text-sm text-slate-600">{license.message}</p>
            </div>
            {license.trialDay !== null && license.state === 'trial' && (
              <div className="text-right">
                <p className="num text-3xl font-semibold text-slate-900">{Math.max(0, license.trialDaysLeft ?? 0)}</p>
                <p className="text-2xs uppercase tracking-wider text-slate-500">days left</p>
              </div>
            )}
          </div>
          {license.clockWarning && (
            <p className="mt-4 flex gap-2 rounded-xl bg-amber-50 px-3 py-2 text-sm text-amber-900">
              <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
              {license.clockWarning}
            </p>
          )}
          <div className="mt-5 border-t border-slate-100 pt-4">
            <LicenseFacts license={license} />
          </div>
        </section>
        <section className="card p-6">
          <h3 className="font-semibold text-slate-900">{license.state === 'licensed' ? 'Renew or upgrade' : 'Activate CODE X MART'}</h3>
          <p className="mb-4 mt-1 text-sm text-slate-500">
            1. Send the request code to the software owner. 2. Enter the activation key you receive. Everything works offline.
          </p>
          <RequestCodePanel />
          <div className="mt-6 border-t border-slate-100 pt-5">
            <ActivationForm
              onActivated={(s: LicenseStatus) => {
                setLicense(s);
                onActivated?.();
              }}
            />
          </div>
        </section>
      </div>
      <OwnerContactCard owner={license.owner} className="self-start" />
    </div>
  );
}

export function LicenseDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { license } = useSession();
  return (
    <Modal
      open={open}
      onClose={onClose}
      size="xl"
      title={license?.headline ?? 'License'}
      icon={<KeyRound className="h-5 w-5" />}
      footer={
        <Button variant="primary" onClick={onClose}>
          {license?.reminder ? 'Remind me tomorrow' : 'Close'}
        </Button>
      }
    >
      <LicensePanel onActivated={onClose} />
    </Modal>
  );
}

/**
 * From day 25 of the trial (or the last 5 days of a license): a chip in the
 * header, and the dialog once a day after signing in. Nothing on days 1-24.
 */
export function LicenseReminder() {
  const { license } = useSession();
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (license?.reminder && readStorage<string | null>(REMINDER_KEY, null) !== license.today) {
      writeStorage(REMINDER_KEY, license.today);
      setOpen(true);
    }
  }, [license?.reminder, license?.today]);

  if (!license) return null;
  const left = license.state === 'licensed' ? license.licenseDaysLeft : license.trialDaysLeft;
  return (
    <>
      {license.reminder && (
        <button
          data-testid="license-chip"
          onClick={() => setOpen(true)}
          className="inline-flex items-center gap-1.5 rounded-full bg-amber-50 px-2.5 py-1 font-medium text-amber-800 ring-1 ring-inset ring-amber-200 hover:bg-amber-100"
        >
          <CalendarClock className="h-3.5 w-3.5" />
          {license.state === 'licensed' ? 'License' : 'Trial'}: {left === 0 ? 'ends today' : `${left} day${left === 1 ? '' : 's'} left`}
        </button>
      )}
      {license.clockWarning && !license.reminder && (
        <span
          title={license.clockWarning}
          className="inline-flex items-center gap-1.5 rounded-full bg-amber-50 px-2.5 py-1 font-medium text-amber-800 ring-1 ring-inset ring-amber-200"
        >
          <TriangleAlert className="h-3.5 w-3.5" />
          Check date &amp; time
        </span>
      )}
      <LicenseDialog open={open} onClose={() => setOpen(false)} />
    </>
  );
}
