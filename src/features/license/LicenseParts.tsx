import { useEffect, useState } from 'react';
import { Check, Copy, KeyRound, MessageCircle, Phone, ScanLine } from 'lucide-react';
import { api, errorMessage, type KeyInspection, type LicenseStatus, type OwnerContact, type RequestCodeInfo } from '@/lib/api';
import { date } from '@/lib/format';
import { copyText, cx, useDebounced } from '@/lib/utils';
import { Button, Field, Textarea, useToast } from '@/components/ui';

function CopyButton({ text, label, size = 'sm' }: { text: string; label: string; size?: 'sm' | 'md' }) {
  const [done, setDone] = useState(false);
  return (
    <Button
      size={size}
      icon={done ? <Check className="h-4 w-4 text-emerald-600" /> : <Copy className="h-4 w-4" />}
      onClick={async () => {
        if (await copyText(text)) {
          setDone(true);
          setTimeout(() => setDone(false), 2000);
        }
      }}
    >
      {done ? 'Copied' : label}
    </Button>
  );
}

/** The software owner's name, number and WhatsApp button. */
export function OwnerContactCard({ owner, className }: { owner: OwnerContact; className?: string }) {
  const toast = useToast();
  return (
    <div className={cx('rounded-2xl border border-slate-200 bg-white p-5 shadow-card', className)}>
      <p className="text-2xs font-semibold uppercase tracking-wider text-slate-500">Software owner</p>
      <p className="mt-1 text-lg font-semibold text-slate-900">{owner.name}</p>
      <p className="num mt-0.5 flex items-center gap-2 text-xl font-semibold tracking-wide text-slate-900">
        <Phone className="h-4 w-4 text-slate-400" />
        {owner.phone}
      </p>
      <div className="mt-4 flex flex-wrap gap-2">
        <Button
          className="!bg-[#1FA855] !text-white hover:!bg-[#178E47]"
          icon={<MessageCircle className="h-4 w-4" />}
          onClick={() => api.license.openWhatsApp().catch((e) => toast.error(e))}
        >
          Contact on WhatsApp
        </Button>
        <CopyButton text={owner.phone} label="Copy number" size="md" />
      </div>
      <p className="mt-3 text-xs text-slate-500">
        WhatsApp opens in your web browser when this computer is online. Without internet, call or message this number from
        your phone.
      </p>
    </div>
  );
}

/** Shows this computer's license request code, to send to the owner. */
export function RequestCodePanel({ autoShow = false }: { autoShow?: boolean }) {
  const toast = useToast();
  const [info, setInfo] = useState<RequestCodeInfo | null>(null);
  const [loading, setLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    try {
      setInfo(await api.license.requestCode());
    } catch (e) {
      toast.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (autoShow) void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoShow]);

  if (!info) {
    return (
      <Button variant="primary" icon={<ScanLine className="h-4 w-4" />} loading={loading} onClick={load}>
        Generate / View License Request Code
      </Button>
    );
  }
  return (
    <div>
      <p className="label">License request code</p>
      <div className="rounded-xl border border-brand-200 bg-brand-50/60 px-4 py-3">
        <p data-testid="request-code" className="select-all break-words font-mono text-lg font-semibold tracking-wider text-brand-900">
          {info.requestCode}
        </p>
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <CopyButton text={info.requestCode} label="Copy Request Code" size="md" />
      </div>
      <dl className="mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-slate-500">Device ID</dt>
        <dd data-testid="device-id" className="font-mono text-slate-800">{info.deviceId}</dd>
        <dt className="text-slate-500">Product</dt>
        <dd className="text-slate-800">
          {info.product} {info.appVersion}
        </dd>
      </dl>
      <p className="mt-3 text-xs text-slate-500">
        Send this code to the software owner on WhatsApp, or read it out on the phone. A photo of this screen also works. The
        code identifies this computer only; it contains no shop data.
      </p>
    </div>
  );
}

/** Activation key entry with live, per-group typing checks. */
export function ActivationForm({ onActivated }: { onActivated: (s: LicenseStatus) => void }) {
  const toast = useToast();
  const [key, setKey] = useState('');
  const [inspection, setInspection] = useState<KeyInspection | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const debounced = useDebounced(key, 120);

  useEffect(() => {
    let live = true;
    if (!debounced.trim()) {
      setInspection(null);
      return;
    }
    api.license
      .inspectKey(debounced)
      .then((i) => live && setInspection(i))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [debounced]);

  const activate = async () => {
    setBusy(true);
    setError(null);
    try {
      const status = await api.license.activate(key);
      toast.success('License activated', status.message);
      setKey('');
      onActivated(status);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const groups = inspection?.groups ?? [];
  const done = groups.filter((g) => g.ok).length;
  return (
    <div>
      <Field label="Activation key" hint="Type or paste the key exactly as received. Capital letters, spaces and dashes do not matter.">
        <Textarea
          rows={4}
          value={key}
          spellCheck={false}
          placeholder="CXMA-XXXXX-XXXXX-XXXXX-…"
          className="font-mono text-sm uppercase tracking-wider"
          onChange={(e) => {
            setKey(e.target.value);
            setError(null);
          }}
        />
      </Field>
      {groups.length > 0 && (
        <div className="mt-3">
          <div className="flex flex-wrap gap-1.5">
            {groups.map((g, i) => (
              <span
                key={i}
                title={`Group ${i + 1}`}
                className={cx(
                  'rounded-md px-1.5 py-0.5 font-mono text-2xs font-semibold ring-1 ring-inset',
                  g.ok === true && 'bg-emerald-50 text-emerald-700 ring-emerald-200',
                  g.ok === false && 'bg-rose-50 text-rose-700 ring-rose-300',
                  g.ok === null && 'bg-slate-50 text-slate-500 ring-slate-200',
                )}
              >
                {g.text}
              </span>
            ))}
          </div>
          <p className="mt-2 text-xs text-slate-500">
            {done} of {inspection?.expectedGroups} groups checked
          </p>
        </div>
      )}
      {inspection?.message && inspection.problem !== 'empty' && inspection.problem !== 'incomplete' && (
        <p className="mt-2 text-sm font-medium text-rose-600">{inspection.message}</p>
      )}
      {error && (
        <div role="alert" className="mt-3 rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800">
          {error}
        </div>
      )}
      <Button
        variant="primary"
        size="lg"
        className="mt-4"
        icon={<KeyRound className="h-4 w-4" />}
        disabled={!inspection?.complete}
        loading={busy}
        onClick={activate}
      >
        Activate License
      </Button>
    </div>
  );
}

/** Trial and license facts as label/value rows. */
export function LicenseFacts({ license }: { license: LicenseStatus }) {
  const rows: [string, string][] = [];
  if (license.license) {
    rows.push(['License', `${license.license.editionLabel} · ${license.license.serial}`]);
    rows.push(['Issued on', date(license.license.issuedOn)]);
    rows.push(['Valid until', license.license.expiresOn ? date(license.license.expiresOn) : 'No expiry (lifetime)']);
  }
  if (license.trialStartedOn) {
    rows.push(['Free trial started', date(license.trialStartedOn)]);
    if (license.trialLastDay) rows.push(['Free trial last day', date(license.trialLastDay)]);
  }
  rows.push(['Device ID', license.deviceId]);
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-slate-500">{k}</dt>
          <dd className="font-medium text-slate-900">{v}</dd>
        </div>
      ))}
    </dl>
  );
}
