import { useState } from 'react';
import { KeyRound } from 'lucide-react';
import { api } from '@/lib/api';
import { Button, Field, Input, Modal, useToast } from '@/components/ui';

export function ChangePasswordDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const toast = useToast();
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);

  const close = () => {
    setCurrent('');
    setNext('');
    setConfirm('');
    onClose();
  };

  const save = async () => {
    setBusy(true);
    try {
      await api.auth.changePassword(current, next);
      toast.success('Password changed');
      close();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={close}
      title="Change your password"
      size="sm"
      icon={<KeyRound className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!current || next.length < 4 || next !== confirm} onClick={save}>
            Change password
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field label="Current password">
          <Input type="password" value={current} onChange={(e) => setCurrent(e.target.value)} />
        </Field>
        <Field label="New password" hint="At least 4 characters.">
          <Input type="password" value={next} onChange={(e) => setNext(e.target.value)} />
        </Field>
        <Field label="Repeat new password" error={confirm && confirm !== next ? 'Passwords do not match' : null}>
          <Input type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
        </Field>
      </div>
    </Modal>
  );
}
