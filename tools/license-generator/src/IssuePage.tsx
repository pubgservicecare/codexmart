import { useEffect, useState } from 'react';
import { Check, ClipboardCheck, Copy, MessageCircle, ScanSearch, Stamp, TriangleAlert } from 'lucide-react';
import { Badge, Button, Field, Input, Modal, PageHeader, Select, Textarea, useToast } from '@/components/ui';
import { copyText } from '@/lib/utils';
import { EDITIONS, errorText, gen, type IssuePreview, type IssueRequest, type IssueResult, type IssuedRecord, type RequestPreview } from './api';

const today = () => new Date().toLocaleDateString('en-CA');

function CopyButton({ text, label }: { text: string; label: string }) {
  const [done, setDone] = useState(false);
  return (
    <Button
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

export function IssuePage({
  prefill,
  onIssued,
  clearPrefill,
}: {
  prefill: { record: IssuedRecord; mode: 'reissue' | 'transfer' } | null;
  onIssued: () => void;
  clearPrefill: () => void;
}) {
  const toast = useToast();
  const blank: IssueRequest = { requestCode: '', customerName: '', customerPhone: '', edition: 'year1', issuedOn: today(), expiresOn: null, notes: '', kind: 'new', replaces: null };
  const [form, setForm] = useState<IssueRequest>(blank);
  const [request, setRequest] = useState<RequestPreview | null>(null);
  const [requestError, setRequestError] = useState<string | null>(null);
  const [review, setReview] = useState<IssuePreview | null>(null);
  const [result, setResult] = useState<IssueResult | null>(null);
  const [busy, setBusy] = useState(false);
  const set = (patch: Partial<IssueRequest>) => setForm((f) => ({ ...f, ...patch }));

  useEffect(() => {
    if (!prefill) return;
    const r = prefill.record;
    setForm({
      ...blank,
      customerName: r.customerName,
      customerPhone: r.customerPhone,
      edition: r.edition,
      expiresOn: r.edition === 'custom' ? r.expiresOn : null,
      kind: prefill.mode,
      replaces: prefill.mode === 'transfer' ? r.serial : null,
      requestCode: prefill.mode === 'reissue' ? r.requestCode : '',
      notes: prefill.mode === 'transfer' ? `Moved from ${r.serial}` : '',
    });
    setRequest(null);
    setResult(null);
    clearPrefill();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefill]);

  const check = async () => {
    setRequestError(null);
    setRequest(null);
    try {
      const r = await gen.decodeRequest(form.requestCode);
      setRequest(r);
      if (r.previous.some((p) => p.status === 'issued') && form.kind === 'new') set({ kind: 'reissue' });
    } catch (e) {
      setRequestError(errorText(e));
    }
  };

  const openReview = async () => {
    setBusy(true);
    try {
      setReview(await gen.review(form));
    } catch (e) {
      toast.error(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const issue = async () => {
    setBusy(true);
    try {
      const r = await gen.issue(form);
      setResult(r);
      setReview(null);
      onIssued();
    } catch (e) {
      toast.error(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  if (result) {
    const r = result.record;
    return (
      <div className="space-y-5">
        <PageHeader title="Activation key issued" description={`${r.editionLabel} for ${r.customerName}`} icon={<ClipboardCheck className="h-5 w-5" />} />
        <section className="card p-6">
          <p className="label">Activation key</p>
          <p data-testid="activation-key" className="select-all break-words rounded-xl bg-slate-50 px-4 py-3 font-mono text-base font-semibold leading-relaxed tracking-wider text-slate-900">
            {r.activationKey}
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <CopyButton text={r.activationKey} label="Copy activation key" />
            <CopyButton text={result.whatsappMessage} label="Copy WhatsApp message" />
          </div>
          <dl className="mt-5 grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
            <dt className="text-slate-500">License no.</dt>
            <dd data-testid="serial" className="font-mono font-semibold">{r.serial}</dd>
            <dt className="text-slate-500">Customer</dt>
            <dd>
              {r.customerName} (no. {r.customerNo}) {r.customerPhone}
            </dd>
            <dt className="text-slate-500">Computer</dt>
            <dd className="font-mono">{r.deviceId}</dd>
            <dt className="text-slate-500">Valid</dt>
            <dd>
              {r.issuedOn} – {r.expiresOn ?? 'no expiry'}
            </dd>
          </dl>
          <div className="mt-5 flex items-start gap-2 rounded-xl bg-emerald-50 px-3 py-2 text-sm text-emerald-900">
            <MessageCircle className="mt-0.5 h-4 w-4 shrink-0" />
            Send the key on WhatsApp. The customer enters it in CODE X MART and presses Activate License; it only works on the computer
            that made the request code.
          </div>
        </section>
        <Button
          variant="primary"
          onClick={() => {
            setResult(null);
            setRequest(null);
            setForm(blank);
          }}
        >
          Issue another license
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <PageHeader title="Issue a license" description="Paste the customer's request code, choose the license, review, then generate the activation key." icon={<Stamp className="h-5 w-5" />} />
      <section className="card p-6">
        <h2 className="font-semibold text-slate-900">1. Customer's request code</h2>
        <div className="mt-3 flex gap-2">
          <Field className="flex-1">
            <Input
              aria-label="Request code"
              className="font-mono uppercase tracking-wider"
              placeholder="CXMR-XXXXX-XXXXX-…"
              value={form.requestCode}
              onChange={(e) => {
                set({ requestCode: e.target.value });
                setRequest(null);
              }}
            />
          </Field>
          <Button icon={<ScanSearch className="h-4 w-4" />} onClick={check} disabled={!form.requestCode.trim()}>
            Check request
          </Button>
        </div>
        {requestError && <p className="mt-2 text-sm font-medium text-rose-600">{requestError}</p>}
        {request && (
          <div className="mt-4 rounded-xl border border-slate-200 bg-slate-50 p-4 text-sm">
            <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1">
              <dt className="text-slate-500">Computer (device ID)</dt>
              <dd data-testid="request-device" className="font-mono font-semibold">{request.deviceId}</dd>
              <dt className="text-slate-500">CODE X MART version</dt>
              <dd>{request.appVersion}</dd>
              <dt className="text-slate-500">Status</dt>
              <dd>
                {request.statusLabel}
                {request.trialStartedOn && ` · trial started ${request.trialStartedOn}`}
                {request.clockWarning && <Badge tone="amber" className="ml-2">clock was set back</Badge>}
              </dd>
            </dl>
            {request.previous.length > 0 && (
              <div className="mt-3 rounded-lg bg-amber-50 px-3 py-2 text-amber-900">
                <p className="flex items-center gap-1.5 font-medium">
                  <TriangleAlert className="h-4 w-4" /> Licenses already issued for this computer
                </p>
                <ul className="mt-1 list-disc pl-5">
                  {request.previous.map((p) => (
                    <li key={p.serial}>
                      {p.serial} · {p.editionLabel} · {p.customerName} · {p.status}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}
      </section>

      <section className="card p-6">
        <h2 className="font-semibold text-slate-900">2. License</h2>
        <div className="mt-3 grid gap-4 md:grid-cols-2">
          <Field label="Customer name or reference" required>
            <Input value={form.customerName} onChange={(e) => set({ customerName: e.target.value })} />
          </Field>
          <Field label="Customer phone">
            <Input value={form.customerPhone} onChange={(e) => set({ customerPhone: e.target.value })} />
          </Field>
          <Field label="License type">
            <Select value={form.edition} onChange={(e) => set({ edition: e.target.value })}>
              {EDITIONS.map((e) => (
                <option key={e.code} value={e.code}>
                  {e.label}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Issue date">
            <Input type="date" value={form.issuedOn} onChange={(e) => set({ issuedOn: e.target.value })} />
          </Field>
          {form.edition === 'custom' && (
            <Field label="Expiry date (last valid day)">
              <Input type="date" value={form.expiresOn ?? ''} onChange={(e) => set({ expiresOn: e.target.value })} />
            </Field>
          )}
          <Field label="Kind">
            <Select value={form.kind} onChange={(e) => set({ kind: e.target.value as IssueRequest['kind'] })}>
              <option value="new">New license</option>
              <option value="reissue">Renew or reissue for this computer</option>
              <option value="transfer">Transfer to a new computer</option>
            </Select>
          </Field>
          {form.kind === 'transfer' && (
            <Field label="License being moved (serial)" hint="Open License history and choose Transfer to fill this in.">
              <Input className="font-mono" value={form.replaces ?? ''} onChange={(e) => set({ replaces: e.target.value })} />
            </Field>
          )}
          <Field label="Internal notes" className="md:col-span-2" hint="Only stored in your history; not part of the key.">
            <Textarea rows={2} value={form.notes} onChange={(e) => set({ notes: e.target.value })} />
          </Field>
        </div>
        <Button className="mt-4" variant="primary" loading={busy} onClick={openReview} disabled={!form.requestCode.trim() || !form.customerName.trim()}>
          Review license
        </Button>
      </section>

      <Modal
        open={!!review}
        onClose={() => setReview(null)}
        title="Check before issuing"
        icon={<Stamp className="h-5 w-5" />}
        footer={
          <>
            <Button onClick={() => setReview(null)}>Back</Button>
            <Button variant="primary" loading={busy} onClick={issue}>
              Generate activation key
            </Button>
          </>
        }
      >
        {review && (
          <div className="space-y-3 text-sm">
            <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5">
              <dt className="text-slate-500">Customer</dt>
              <dd className="font-medium">
                {review.customerName} {review.customerPhone}
              </dd>
              <dt className="text-slate-500">Computer</dt>
              <dd className="font-mono">{review.deviceId}</dd>
              <dt className="text-slate-500">License</dt>
              <dd className="font-medium">{review.editionLabel}</dd>
              <dt className="text-slate-500">Issued</dt>
              <dd>{review.issuedOn}</dd>
              <dt className="text-slate-500">Valid until</dt>
              <dd className="font-medium">{review.expiresOn ?? 'No expiry (lifetime)'}</dd>
              <dt className="text-slate-500">Kind</dt>
              <dd>
                {review.kind}
                {review.replaces ? ` (replaces ${review.replaces})` : ''}
              </dd>
            </dl>
            {review.warnings.map((w) => (
              <p key={w} className="flex gap-2 rounded-lg bg-amber-50 px-3 py-2 text-amber-900">
                <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
                {w}
              </p>
            ))}
          </div>
        )}
      </Modal>
    </div>
  );
}
