import { useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { HardDriveDownload, KeyRound } from 'lucide-react';
import { Button, Field, Input, Modal, PageHeader, useToast } from '@/components/ui';
import { errorText, gen, type GenStatus } from './api';

export function KeyPage({ status, onChange }: { status: GenStatus; onChange: () => void }) {
  const toast = useToast();
  const key = status.key!;
  const [pwOpen, setPwOpen] = useState(false);
  const [pw, setPw] = useState({ current: '', next: '', confirm: '' });

  const backup = async () => {
    const stamp = new Date().toLocaleDateString('en-CA');
    const path = await save({
      title: 'Save signing key backup (choose a USB drive)',
      defaultPath: `codexmart-signing-key-${key.fingerprint.slice(0, 4)}-${stamp}.cxkey`,
      filters: [{ name: 'Signing key backup', extensions: ['cxkey'] }],
    });
    if (!path) return;
    try {
      await gen.backupKey(path);
      toast.success('Key backup saved and checked', path);
      onChange();
    } catch (e) {
      toast.error(errorText(e));
    }
  };

  return (
    <div className="space-y-5">
      <PageHeader title="Signing key" description="The private key that signs every activation key. It never leaves this computer unencrypted." icon={<KeyRound className="h-5 w-5" />} />
      <section className="card p-6">
        <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
          <dt className="text-slate-500">Key fingerprint</dt>
          <dd data-testid="key-fingerprint" className="font-mono font-semibold">{key.fingerprint}</dd>
          <dt className="text-slate-500">Key id</dt>
          <dd>{key.keyId}</dd>
          <dt className="text-slate-500">Public key</dt>
          <dd data-testid="public-key" className="select-all break-all font-mono text-xs">{key.publicKey}</dd>
          <dt className="text-slate-500">Created</dt>
          <dd>{key.createdAt}</dd>
          <dt className="text-slate-500">Trusted by CODE X MART {status.version}</dt>
          <dd className={key.trustedByApp ? 'text-emerald-700' : 'font-semibold text-rose-700'}>{key.trustedByApp ? 'Yes' : 'No'}</dd>
          <dt className="text-slate-500">Last backup</dt>
          <dd className={key.lastBackupAt ? '' : 'font-semibold text-amber-700'}>{key.lastBackupAt ?? 'Never: save one now'}</dd>
          <dt className="text-slate-500">Data folder</dt>
          <dd className="break-all font-mono text-xs">{status.dataDir}</dd>
        </dl>
        <div className="mt-5 flex flex-wrap gap-2">
          <Button variant="primary" icon={<HardDriveDownload className="h-4 w-4" />} onClick={backup}>
            Save key backup…
          </Button>
          <Button onClick={() => setPwOpen(true)}>Change passphrase</Button>
        </div>
      </section>
      <section className="card p-6 text-sm text-slate-700">
        <h2 className="font-semibold text-slate-900">Keeping the key safe</h2>
        <ul className="mt-2 list-disc space-y-1 pl-5">
          <li>Save the backup on two USB drives and keep them in different places (for example the shop and home).</li>
          <li>Write the passphrase on paper and keep it apart from the USB drives. Without the passphrase the backup cannot be opened.</li>
          <li>Never send the backup file or passphrase to anyone, and never put them in the CODE X MART source code, GitHub or a customer's computer.</li>
          <li>On a new computer, install the License Generator and choose Restore from a key backup.</li>
          <li>If the key and every backup are lost, existing customers keep working, but new licenses need a new key and an updated CODE X MART installer.</li>
        </ul>
      </section>
      <Modal
        open={pwOpen}
        onClose={() => setPwOpen(false)}
        title="Change passphrase"
        footer={
          <Button
            variant="primary"
            onClick={async () => {
              try {
                await gen.changePassphrase(pw.current, pw.next, pw.confirm);
                toast.success('Passphrase changed', 'Save a new key backup: old backups still use the old passphrase.');
                setPwOpen(false);
                setPw({ current: '', next: '', confirm: '' });
              } catch (e) {
                toast.error(errorText(e));
              }
            }}
          >
            Change passphrase
          </Button>
        }
      >
        <div className="space-y-3">
          <Field label="Current passphrase">
            <Input type="password" value={pw.current} onChange={(e) => setPw({ ...pw, current: e.target.value })} />
          </Field>
          <Field label="New passphrase">
            <Input type="password" value={pw.next} onChange={(e) => setPw({ ...pw, next: e.target.value })} />
          </Field>
          <Field label="Repeat new passphrase">
            <Input type="password" value={pw.confirm} onChange={(e) => setPw({ ...pw, confirm: e.target.value })} />
          </Field>
        </div>
      </Modal>
    </div>
  );
}
