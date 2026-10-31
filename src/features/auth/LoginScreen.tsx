import { useState, type FormEvent } from 'react';
import { LifeBuoy, LogIn } from 'lucide-react';
import { api, errorMessage } from '@/lib/api';
import { useSession } from '@/lib/session';
import { Button, Field, Input, Modal, useToast } from '@/components/ui';
import { AuthLayout } from './AuthLayout';

export function LoginScreen() {
  const { setSession, status } = useSession();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [recoverOpen, setRecoverOpen] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      setSession(await api.auth.login(username, password));
    } catch (err) {
      setError(errorMessage(err));
      setPassword('');
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthLayout>
      <div className="mb-8 lg:hidden">
        <img src="/logo.svg" alt="" className="h-12 w-12" />
      </div>
      <h2 className="text-2xl font-semibold tracking-tight text-slate-900">Sign in</h2>
      <p className="mt-1 text-sm text-slate-500">Welcome back. Enter your username and password.</p>
      <form onSubmit={submit} className="mt-8 space-y-4">
        <Field label="Username">
          <Input autoFocus value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" />
        </Field>
        <Field label="Password">
          <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" />
        </Field>
        {error && <p className="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700">{error}</p>}
        <Button type="submit" variant="primary" size="lg" className="w-full" loading={busy} icon={<LogIn className="h-4 w-4" />} disabled={!username || !password}>
          Sign in
        </Button>
      </form>
      <button className="mt-6 inline-flex items-center gap-1.5 text-sm text-slate-500 hover:text-brand-700" onClick={() => setRecoverOpen(true)}>
        <LifeBuoy className="h-4 w-4" /> Forgot the administrator password?
      </button>
      <p className="mt-10 text-xs text-slate-400">
        {status?.appName} v{status?.appVersion}
      </p>
      <RecoverDialog open={recoverOpen} onClose={() => setRecoverOpen(false)} />
    </AuthLayout>
  );
}

function RecoverDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const toast = useToast();
  const [key, setKey] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      await api.auth.recover(key, username, password);
      toast.success('Password reset', 'Sign in with the new password.');
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Reset administrator password"
      description="Use the recovery key that was shown when CODE X MART was first set up."
      icon={<LifeBuoy className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={!key || !username || password.length < 4 || password !== confirm}
            onClick={submit}
          >
            Reset password
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field label="Recovery key" hint="Format: XXXXX-XXXXX-XXXXX-XXXXX">
          <Input value={key} onChange={(e) => setKey(e.target.value.toUpperCase())} className="font-mono tracking-wider" />
        </Field>
        <Field label="Administrator username">
          <Input value={username} onChange={(e) => setUsername(e.target.value)} />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="New password">
            <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </Field>
          <Field label="Repeat password" error={confirm && confirm !== password ? 'Passwords do not match' : null}>
            <Input type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
          </Field>
        </div>
      </div>
    </Modal>
  );
}
