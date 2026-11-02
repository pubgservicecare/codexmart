import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { FolderOpen, KeyRound, Pencil, Plus, Printer, ScrollText, Settings as SettingsIcon, ShieldCheck, UserPlus } from 'lucide-react';
import { api, type LookupKind, type Role, type SaleDetail, type Settings, type User } from '@/lib/api';
import { bytes, dateTime, percent } from '@/lib/format';
import { usePrint } from '@/lib/print';
import { useSession, useSettings } from '@/lib/session';
import { cx, useDebounced } from '@/lib/utils';
import {
  Badge,
  Button,
  EmptyState,
  Field,
  Input,
  Modal,
  PageHeader,
  Pager,
  SearchBox,
  Select,
  Spinner,
  Switch,
  Tabs,
  Textarea,
  useToast,
} from '@/components/ui';
import { Receipt } from '@/components/Receipt';
import { ChangePasswordDialog } from '@/features/auth/ChangePasswordDialog';
import { LicensePanel } from '@/features/license/LicenseReminder';

type Tab = 'store' | 'receipt' | 'features' | 'users' | 'lists' | 'security' | 'audit' | 'license' | 'about';

export function SettingsPage() {
  const { can } = useSession();
  const tabs = useMemo(
    () =>
      [
        can('manageSettings') && { key: 'store' as Tab, label: 'Store' },
        can('manageSettings') && { key: 'receipt' as Tab, label: 'Receipt & printer' },
        can('manageSettings') && { key: 'features' as Tab, label: 'Features & rules' },
        can('manageUsers') && { key: 'users' as Tab, label: 'Users' },
        (can('manageCatalog') || can('manageExpenses')) && { key: 'lists' as Tab, label: 'Categories & units' },
        (can('manageUsers') || can('manageInventory')) && { key: 'security' as Tab, label: 'Security & health' },
        can('viewAudit') && { key: 'audit' as Tab, label: 'Audit log' },
        { key: 'license' as Tab, label: 'License' },
        { key: 'about' as Tab, label: 'About' },
      ].filter(Boolean) as { key: Tab; label: string }[],
    [can],
  );
  const [tab, setTab] = useState<Tab>(tabs[0].key);

  return (
    <div>
      <PageHeader title="Settings" description="Store details, receipts, features, users and system information." icon={<SettingsIcon className="h-5 w-5" />} />
      <Tabs className="mb-5" tabs={tabs} value={tab} onChange={setTab} />
      {tab === 'store' && <StoreSettings />}
      {tab === 'receipt' && <ReceiptSettings />}
      {tab === 'features' && <FeatureSettings />}
      {tab === 'users' && <UsersSettings />}
      {tab === 'lists' && <ListsSettings />}
      {tab === 'security' && <SecuritySettings />}
      {tab === 'audit' && <AuditLog />}
      {tab === 'license' && <LicensePanel />}
      {tab === 'about' && <About />}
    </div>
  );
}

/** Edits a copy of the settings and saves the whole object. */
function useSettingsForm() {
  const settings = useSettings();
  const qc = useQueryClient();
  const toast = useToast();
  const [form, setForm] = useState<Settings>(settings);
  const [busy, setBusy] = useState(false);
  useEffect(() => setForm(settings), [settings]);
  const dirty = JSON.stringify(form) !== JSON.stringify(settings);
  const set = (p: Partial<Settings>) => setForm((f) => ({ ...f, ...p }));
  const save = async () => {
    setBusy(true);
    try {
      await api.settings.save(form);
      await qc.invalidateQueries({ queryKey: ['settings'] });
      toast.success('Settings saved');
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };
  return { form, set, save, busy, dirty, reset: () => setForm(settings) };
}

function Section({ title, description, children, footer }: { title: string; description?: string; children: ReactNode; footer?: ReactNode }) {
  return (
    <div className="card">
      <div className="border-b border-slate-100 px-6 py-4">
        <h2 className="card-title">{title}</h2>
        {description && <p className="mt-0.5 text-xs text-slate-500">{description}</p>}
      </div>
      <div className="p-6">{children}</div>
      {footer && <div className="flex justify-end gap-2 border-t border-slate-100 bg-slate-50/60 px-6 py-3">{footer}</div>}
    </div>
  );
}

function SaveBar({ f }: { f: ReturnType<typeof useSettingsForm> }) {
  return (
    <>
      <Button disabled={!f.dirty} onClick={f.reset}>
        Discard
      </Button>
      <Button variant="primary" disabled={!f.dirty} loading={f.busy} onClick={f.save}>
        Save changes
      </Button>
    </>
  );
}

function StoreSettings() {
  const f = useSettingsForm();
  return (
    <Section title="Store details" description="Shown on receipts, invoices and reports." footer={<SaveBar f={f} />}>
      <div className="grid max-w-3xl grid-cols-2 gap-4">
        <Field label="Store name" required>
          <Input value={f.form.storeName} onChange={(e) => f.set({ storeName: e.target.value })} />
        </Field>
        <Field label="Tagline">
          <Input value={f.form.storeTagline} onChange={(e) => f.set({ storeTagline: e.target.value })} placeholder="e.g. Cosmetics · Bags · General Items" />
        </Field>
        <Field label="Phone">
          <Input value={f.form.storePhone} onChange={(e) => f.set({ storePhone: e.target.value })} />
        </Field>
        <Field label="Email">
          <Input value={f.form.storeEmail} onChange={(e) => f.set({ storeEmail: e.target.value })} />
        </Field>
        <Field label="Address" className="col-span-2">
          <Textarea rows={2} value={f.form.storeAddress} onChange={(e) => f.set({ storeAddress: e.target.value })} />
        </Field>
        <Field label="Tax / NTN number">
          <Input value={f.form.storeTaxId} onChange={(e) => f.set({ storeTaxId: e.target.value })} />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Currency symbol">
            <Input value={f.form.currencySymbol} onChange={(e) => f.set({ currencySymbol: e.target.value })} />
          </Field>
          <Field label="Show decimals">
            <Select value={f.form.currencyDecimals} onChange={(e) => f.set({ currencyDecimals: Number(e.target.value) })}>
              <option value={2}>Yes (1,250.50)</option>
              <option value={0}>No (1,251)</option>
            </Select>
          </Field>
        </div>
      </div>
    </Section>
  );
}

const SAMPLE_SALE: SaleDetail = {
  id: 'sample',
  invoiceNo: 'INV-000123',
  status: 'completed',
  customerId: null,
  customerName: 'Sample Customer',
  itemCount: 2,
  totalQty: 3000,
  subtotal: 175000,
  discountTotal: 5000,
  total: 170000,
  paidAmount: 170000,
  creditAmount: 0,
  returnedAmount: 0,
  cashierName: 'Cashier',
  businessDate: '2026-01-01',
  createdAt: new Date().toISOString(),
  priceTier: 'retail',
  lineDiscountTotal: 5000,
  billDiscount: 0,
  tenderedCash: 200000,
  changeAmount: 30000,
  cogsTotal: null,
  note: null,
  customerPhone: null,
  voidedAt: null,
  voidReason: null,
  voidedByName: null,
  items: [
    { id: '1', lineNo: 1, productId: 'p1', productName: 'Rose Glow Cream 50ml', sku: 'CXM-00001', barcode: null, unitName: 'pcs', qty: 2000, unitPrice: 45000, grossAmount: 90000, lineDiscount: 5000, billDiscountShare: 0, netAmount: 85000, costAmount: null, returnedQty: 0 },
    { id: '2', lineNo: 2, productId: 'p2', productName: 'Leather Handbag — Black', sku: 'CXM-00002', barcode: null, unitName: 'pcs', qty: 1000, unitPrice: 85000, grossAmount: 85000, lineDiscount: 0, billDiscountShare: 0, netAmount: 85000, costAmount: null, returnedQty: 0 },
  ],
  payments: [{ method: 'cash', amount: 170000, reference: null }],
  returns: [],
};

function ReceiptSettings() {
  const f = useSettingsForm();
  const toast = useToast();
  const print = usePrint();
  const { data: printers, isLoading } = useQuery({ queryKey: ['printers'], queryFn: api.printing.printers });
  const [testing, setTesting] = useState(false);

  const testPrint = async () => {
    setTesting(true);
    try {
      if (f.form.receiptPrintMode === 'escpos') {
        await api.printing.testDirect(f.form.receiptPrinter, f.form.receiptPaper);
        toast.success('Test page sent', f.form.receiptPrinter);
      } else {
        await print(<Receipt sale={SAMPLE_SALE} settings={f.form} paper={f.form.receiptPaper} />, f.form.receiptPaper);
      }
    } catch (e) {
      toast.error(e);
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="grid grid-cols-5 gap-5">
      <div className="col-span-3">
        <Section
          title="Receipts & printing"
          description="Works with ordinary Windows printers and 58 mm / 80 mm thermal receipt printers."
          footer={
            <>
              <Button className="mr-auto" icon={<Printer className="h-4 w-4" />} loading={testing} onClick={testPrint}>
                Print test
              </Button>
              <SaveBar f={f} />
            </>
          }
        >
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <Field label="Paper">
                <Select value={f.form.receiptPaper} onChange={(e) => f.set({ receiptPaper: e.target.value as Settings['receiptPaper'] })}>
                  <option value="80mm">80 mm thermal roll</option>
                  <option value="58mm">58 mm thermal roll</option>
                  <option value="a4">A4 invoice</option>
                </Select>
              </Field>
              <Field label="How to print">
                <Select value={f.form.receiptPrintMode} onChange={(e) => f.set({ receiptPrintMode: e.target.value as Settings['receiptPrintMode'] })}>
                  <option value="dialog">Windows print dialog (any printer)</option>
                  <option value="escpos">Direct to thermal printer (ESC/POS, no dialog)</option>
                </Select>
              </Field>
            </div>
            {f.form.receiptPrintMode === 'escpos' && (
              <div className="grid grid-cols-2 gap-4">
                <Field label="Receipt printer" hint={isLoading ? 'Looking for printers…' : printers?.length ? undefined : 'No printers found. Install the printer driver in Windows first.'}>
                  <Select value={f.form.receiptPrinter} onChange={(e) => f.set({ receiptPrinter: e.target.value })}>
                    <option value="">Choose…</option>
                    {printers?.map((p) => (
                      <option key={p.name} value={p.name}>
                        {p.name}
                        {p.isDefault ? ' (default)' : ''}
                      </option>
                    ))}
                  </Select>
                </Field>
                <Field label="Copies">
                  <Select value={f.form.receiptCopies} onChange={(e) => f.set({ receiptCopies: Number(e.target.value) })}>
                    {[1, 2, 3].map((n) => (
                      <option key={n} value={n}>
                        {n}
                      </option>
                    ))}
                  </Select>
                </Field>
              </div>
            )}
            <Switch checked={f.form.autoPrintReceipt} onChange={(v) => f.set({ autoPrintReceipt: v })} label="Print the receipt automatically after every sale" />
            {f.form.receiptPrintMode === 'escpos' && (
              <Switch checked={f.form.openCashDrawer} onChange={(v) => f.set({ openCashDrawer: v })} label="Open the cash drawer on cash sales" description="For drawers connected to the receipt printer." />
            )}
            <Field label="Receipt header (optional)">
              <Textarea rows={2} value={f.form.receiptHeader} onChange={(e) => f.set({ receiptHeader: e.target.value })} />
            </Field>
            <Field label="Receipt footer">
              <Textarea rows={2} value={f.form.receiptFooter} onChange={(e) => f.set({ receiptFooter: e.target.value })} placeholder="e.g. No return without receipt. Exchange within 7 days." />
            </Field>
            <div className="grid grid-cols-2 gap-4">
              <Field label="Invoice number prefix" hint={`Next invoices look like ${f.form.invoicePrefix || ''}000124`}>
                <Input value={f.form.invoicePrefix} onChange={(e) => f.set({ invoicePrefix: e.target.value.toUpperCase() })} />
              </Field>
              <Field label="Barcode label size">
                <Select value={f.form.labelSize} onChange={(e) => f.set({ labelSize: e.target.value as Settings['labelSize'] })}>
                  <option value="38x25">38 × 25 mm</option>
                  <option value="50x25">50 × 25 mm</option>
                  <option value="a4-65">A4 sheet (65 labels)</option>
                </Select>
              </Field>
            </div>
          </div>
        </Section>
      </div>
      <div className="col-span-2">
        <div className="card p-5">
          <p className="mb-3 text-xs font-semibold uppercase text-slate-500">Preview</p>
          <div className="max-h-[640px] overflow-y-auto rounded-lg bg-slate-100 p-4">
            <div className="mx-auto w-fit bg-white shadow-card">
              <Receipt sale={SAMPLE_SALE} settings={f.form} paper={f.form.receiptPaper === 'a4' ? '80mm' : f.form.receiptPaper} />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function FeatureSettings() {
  const f = useSettingsForm();
  return (
    <Section title="Features & rules" description="Turn on only what your shop needs. Everything keeps working offline." footer={<SaveBar f={f} />}>
      <div className="grid max-w-4xl grid-cols-2 gap-6">
        <Switch
          checked={f.form.enableCreditSales}
          onChange={(v) => f.set({ enableCreditSales: v })}
          label="Credit sales (udhaar)"
          description="Let a named customer pay later. Balances and payments are tracked per customer."
        />
        <Switch
          checked={f.form.enableBatchTracking}
          onChange={(v) => f.set({ enableBatchTracking: v })}
          label="Batch & expiry tracking"
          description="For creams and cosmetics. Turn it on per product; sold first-expiry-first-out."
        />
        <Switch checked={f.form.enableVariants} onChange={(v) => f.set({ enableVariants: v })} label="Sizes & colours" description="Show size and colour/shade fields on products." />
        <Switch checked={f.form.enableWholesale} onChange={(v) => f.set({ enableWholesale: v })} label="Wholesale prices" description="Second price per product, selectable on the billing screen." />
        <Switch
          checked={f.form.allowNegativeStock}
          onChange={(v) => f.set({ allowNegativeStock: v })}
          label="Allow selling more than the recorded stock"
          description="Not recommended: stock and profit figures become less accurate until stock is corrected."
        />
        <div className="grid grid-cols-2 gap-3">
          <Field label="Cashier discount limit" hint={f.form.maxCashierDiscountBp >= 10000 ? 'No limit' : `Up to ${percent(f.form.maxCashierDiscountBp)} of the bill`}>
            <Select value={f.form.maxCashierDiscountBp} onChange={(e) => f.set({ maxCashierDiscountBp: Number(e.target.value) })}>
              {[0, 500, 1000, 1500, 2000, 2500, 5000, 10000].map((bp) => (
                <option key={bp} value={bp}>
                  {bp >= 10000 ? 'No limit' : percent(bp)}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Expiry warning">
            <Select value={f.form.expiryWarningDays} onChange={(e) => f.set({ expiryWarningDays: Number(e.target.value) })}>
              {[15, 30, 60, 90, 180].map((d) => (
                <option key={d} value={d}>
                  {d} days before
                </option>
              ))}
            </Select>
          </Field>
        </div>
      </div>
    </Section>
  );
}

const ROLE_INFO: Record<Role, string> = {
  admin: 'Everything, including users, settings and restoring backups.',
  manager: 'Sales, returns, voids, products, stock, purchases, expenses, reports and backups.',
  cashier: 'Billing, sales history, customers and receiving customer payments.',
};

function UsersSettings() {
  const { session } = useSession();
  const { data, isLoading } = useQuery({ queryKey: ['users'], queryFn: api.users.list });
  const [editing, setEditing] = useState<User | 'new' | null>(null);
  const [resetting, setResetting] = useState<User | null>(null);
  return (
    <Section
      title="Users"
      description="Each person signs in with their own account so every sale and change is traceable."
      footer={
        <Button variant="primary" icon={<UserPlus className="h-4 w-4" />} onClick={() => setEditing('new')}>
          Add user
        </Button>
      }
    >
      {isLoading ? (
        <Spinner />
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Username</th>
              <th>Role</th>
              <th>Last sign-in</th>
              <th>Status</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {data?.map((u) => (
              <tr key={u.id}>
                <td className="font-medium text-slate-900">
                  {u.fullName} {u.id === session?.user.userId && <Badge tone="brand">You</Badge>}
                </td>
                <td className="font-mono text-xs">{u.username}</td>
                <td className="capitalize">{u.role}</td>
                <td className="text-slate-600">{dateTime(u.lastLoginAt)}</td>
                <td>{u.isActive ? <Badge tone="green">Active</Badge> : <Badge>Disabled</Badge>}</td>
                <td className="text-right">
                  <Button size="sm" variant="ghost" icon={<KeyRound className="h-4 w-4" />} onClick={() => setResetting(u)}>
                    Reset password
                  </Button>
                  <Button size="sm" variant="ghost" iconOnly aria-label="Edit" onClick={() => setEditing(u)}>
                    <Pencil className="h-4 w-4" />
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {editing && <UserForm user={editing === 'new' ? null : editing} onClose={() => setEditing(null)} />}
      {resetting && <ResetPasswordDialog user={resetting} onClose={() => setResetting(null)} />}
    </Section>
  );
}

function UserForm({ user, onClose }: { user: User | null; onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const [fullName, setFullName] = useState(user?.fullName ?? '');
  const [username, setUsername] = useState(user?.username ?? '');
  const [role, setRole] = useState<Role>(user?.role ?? 'cashier');
  const [active, setActive] = useState(user?.isActive ?? true);
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const save = async () => {
    setBusy(true);
    try {
      if (user) await api.users.update(user.id, { fullName, role, isActive: active });
      else await api.users.create({ username, fullName, role, password });
      void qc.invalidateQueries({ queryKey: ['users'] });
      toast.success(user ? 'User updated' : 'User added', fullName);
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      open
      onClose={onClose}
      size="sm"
      title={user ? `Edit ${user.fullName}` : 'Add user'}
      icon={<UserPlus className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!fullName.trim() || (!user && (username.length < 3 || password.length < 4))} onClick={save}>
            Save
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field label="Full name" required>
          <Input value={fullName} onChange={(e) => setFullName(e.target.value)} />
        </Field>
        {!user && (
          <>
            <Field label="Username" required>
              <Input value={username} onChange={(e) => setUsername(e.target.value.toLowerCase())} />
            </Field>
            <Field label="Password" required hint="At least 4 characters. The user can change it later.">
              <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
            </Field>
          </>
        )}
        <Field label="Role" hint={ROLE_INFO[role]}>
          <Select value={role} onChange={(e) => setRole(e.target.value as Role)}>
            <option value="cashier">Cashier</option>
            <option value="manager">Manager</option>
            <option value="admin">Administrator</option>
          </Select>
        </Field>
        {user && <Switch checked={active} onChange={setActive} label="Can sign in" />}
      </div>
    </Modal>
  );
}

function ResetPasswordDialog({ user, onClose }: { user: User; onClose: () => void }) {
  const toast = useToast();
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const save = async () => {
    setBusy(true);
    try {
      await api.users.resetPassword(user.id, password);
      toast.success('Password reset', `Give the new password to ${user.fullName}.`);
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      open
      onClose={onClose}
      size="sm"
      title={`Reset password for ${user.fullName}`}
      icon={<KeyRound className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={password.length < 4} onClick={save}>
            Reset password
          </Button>
        </>
      }
    >
      <Field label="New password" hint="At least 4 characters.">
        <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
      </Field>
    </Modal>
  );
}

function ListsSettings() {
  const { can } = useSession();
  return (
    <div className="grid grid-cols-2 gap-5">
      {can('manageCatalog') && <LookupManager kind="category" title="Product categories" />}
      {can('manageCatalog') && <LookupManager kind="brand" title="Brands" />}
      {can('manageCatalog') && <UnitsManager />}
      {can('manageExpenses') && <LookupManager kind="expenseCategory" title="Expense categories" />}
    </div>
  );
}

function LookupManager({ kind, title }: { kind: LookupKind; title: string }) {
  const toast = useToast();
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ['lookups', kind, 'all'], queryFn: () => api.lookups.list(kind, true) });
  const [name, setName] = useState('');
  const [editing, setEditing] = useState<{ id: string; name: string; isActive: boolean } | null>(null);
  const refresh = () => void qc.invalidateQueries({ queryKey: ['lookups', kind] });
  const add = async () => {
    try {
      await api.lookups.create(kind, name);
      setName('');
      refresh();
    } catch (e) {
      toast.error(e);
    }
  };
  const save = async () => {
    if (!editing) return;
    try {
      await api.lookups.update(kind, editing.id, editing.name, editing.isActive);
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(e);
    }
  };
  return (
    <div className="card">
      <div className="card-header">
        <h2 className="card-title">{title}</h2>
      </div>
      <form
        className="flex gap-2 border-b border-slate-100 p-4"
        onSubmit={(e) => {
          e.preventDefault();
          if (name.trim()) void add();
        }}
      >
        <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="New name" />
        <Button type="submit" icon={<Plus className="h-4 w-4" />} disabled={!name.trim()}>
          Add
        </Button>
      </form>
      <div className="max-h-80 overflow-y-auto">
        {!data?.length ? (
          <EmptyState title="Nothing here yet" />
        ) : (
          <table className="table">
            <tbody>
              {data.map((l) => (
                <tr key={l.id}>
                  <td className={cx('font-medium', l.isActive ? 'text-slate-900' : 'text-slate-400 line-through')}>{l.name}</td>
                  <td className="text-right text-xs text-slate-500">{l.usageCount} used</td>
                  <td className="w-12 text-right">
                    <Button size="sm" variant="ghost" iconOnly aria-label="Edit" onClick={() => setEditing({ id: l.id, name: l.name, isActive: l.isActive })}>
                      <Pencil className="h-4 w-4" />
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {editing && (
        <Modal
          open
          onClose={() => setEditing(null)}
          size="sm"
          title={`Edit ${title.toLowerCase().replace(/s$/, '')}`}
          footer={
            <>
              <Button onClick={() => setEditing(null)}>Cancel</Button>
              <Button variant="primary" disabled={!editing.name.trim()} onClick={save}>
                Save
              </Button>
            </>
          }
        >
          <div className="space-y-4">
            <Field label="Name">
              <Input value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.target.value })} />
            </Field>
            <Switch checked={editing.isActive} onChange={(v) => setEditing({ ...editing, isActive: v })} label="Active" description="Inactive entries are hidden from lists but kept on old records." />
          </div>
        </Modal>
      )}
    </div>
  );
}

function UnitsManager() {
  const toast = useToast();
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ['units', 'all'], queryFn: () => api.units.list(true) });
  const [form, setForm] = useState<{ id?: string; name: string; shortName: string; allowDecimal: boolean; isActive: boolean } | null>(null);
  const save = async () => {
    if (!form) return;
    try {
      if (form.id) await api.units.update(form.id, form);
      else await api.units.create(form);
      void qc.invalidateQueries({ queryKey: ['units'] });
      setForm(null);
    } catch (e) {
      toast.error(e);
    }
  };
  return (
    <div className="card">
      <div className="card-header">
        <h2 className="card-title">Units of measurement</h2>
        <Button size="sm" icon={<Plus className="h-4 w-4" />} onClick={() => setForm({ name: '', shortName: '', allowDecimal: false, isActive: true })}>
          Add unit
        </Button>
      </div>
      <div className="max-h-80 overflow-y-auto">
        <table className="table">
          <tbody>
            {data?.map((u) => (
              <tr key={u.id}>
                <td className={cx('font-medium', u.isActive ? 'text-slate-900' : 'text-slate-400 line-through')}>
                  {u.name} <span className="text-slate-500">({u.shortName})</span>
                </td>
                <td className="text-xs text-slate-500">{u.allowDecimal ? 'Decimals (e.g. 1.5)' : 'Whole numbers'}</td>
                <td className="w-12 text-right">
                  <Button size="sm" variant="ghost" iconOnly aria-label="Edit" onClick={() => setForm({ ...u })}>
                    <Pencil className="h-4 w-4" />
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {form && (
        <Modal
          open
          onClose={() => setForm(null)}
          size="sm"
          title={form.id ? 'Edit unit' : 'Add unit'}
          footer={
            <>
              <Button onClick={() => setForm(null)}>Cancel</Button>
              <Button variant="primary" disabled={!form.name.trim() || !form.shortName.trim()} onClick={save}>
                Save
              </Button>
            </>
          }
        >
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-3">
              <Field label="Name">
                <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="e.g. Kilogram" />
              </Field>
              <Field label="Short name">
                <Input value={form.shortName} onChange={(e) => setForm({ ...form, shortName: e.target.value })} placeholder="e.g. kg" />
              </Field>
            </div>
            <Switch checked={form.allowDecimal} onChange={(v) => setForm({ ...form, allowDecimal: v })} label="Allow decimal quantities" description="For items sold by weight or length." />
            {form.id && <Switch checked={form.isActive} onChange={(v) => setForm({ ...form, isActive: v })} label="Active" />}
          </div>
        </Modal>
      )}
    </div>
  );
}

function SecuritySettings() {
  const { can } = useSession();
  const toast = useToast();
  const [key, setKey] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [health, setHealth] = useState<Awaited<ReturnType<typeof api.app.health>> | null>(null);

  const regenerate = async () => {
    try {
      setKey(await api.auth.regenerateRecoveryKey());
    } catch (e) {
      toast.error(e);
    }
  };
  const check = async () => {
    setChecking(true);
    try {
      setHealth(await api.app.health());
    } catch (e) {
      toast.error(e);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="grid grid-cols-2 gap-5">
      {can('manageInventory') && (
        <Section title="Database health check" description="Checks the database file for damage and every product's stock against its full history.">
          <Button icon={<ShieldCheck className="h-4 w-4" />} loading={checking} onClick={check}>
            Run health check
          </Button>
          {health && (
            <div className={cx('mt-4 rounded-lg px-4 py-3 text-sm', health.integrityProblems.length || health.stockMismatches ? 'bg-rose-50 text-rose-800' : 'bg-emerald-50 text-emerald-800')}>
              <p className="font-semibold">{health.integrityProblems.length || health.stockMismatches ? 'Problems found' : 'Everything is healthy'}</p>
              <p>Database file: {health.integrityProblems.length ? health.integrityProblems.join('; ') : 'no damage found'}</p>
              <p>Stock records: {health.stockMismatches ? `${health.stockMismatches} product(s) differ from their history` : 'all consistent'}</p>
              <p className="mt-1 text-xs opacity-80">
                Database version {health.schemaVersion} · {bytes(health.dbSizeBytes)}
              </p>
            </div>
          )}
        </Section>
      )}
      {can('manageUsers') && (
        <Section title="Administrator recovery key" description="Used to reset an administrator password if it is forgotten.">
          <p className="text-sm text-slate-600">Generating a new key makes the old one stop working. Write the new key down and keep it safe.</p>
          <Button className="mt-4" icon={<KeyRound className="h-4 w-4" />} onClick={regenerate}>
            Generate new recovery key
          </Button>
          {key && (
            <div className="mt-4 rounded-xl border-2 border-dashed border-amber-300 bg-amber-50/60 px-4 py-3">
              <code className="select-all font-mono text-lg font-semibold tracking-widest">{key}</code>
              <p className="mt-1 text-xs text-amber-800">Shown only now. Write it down.</p>
            </div>
          )}
        </Section>
      )}
    </div>
  );
}

function AuditLog() {
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const q = useDebounced(search, 250);
  const { data, isLoading } = useQuery({
    queryKey: ['audit', q, offset],
    queryFn: () => api.audit.list({ search: q, limit: 100, offset }),
    placeholderData: keepPreviousData,
  });
  return (
    <div className="card">
      <div className="flex items-center gap-3 border-b border-slate-100 p-4">
        <ScrollText className="h-5 w-5 text-slate-400" />
        <SearchBox value={search} onChange={(v) => { setSearch(v); setOffset(0); }} placeholder="Search the audit log" className="w-96" />
        <p className="ml-auto text-xs text-slate-500">The audit log is permanent and cannot be edited.</p>
      </div>
      {isLoading ? (
        <Spinner />
      ) : !data?.items.length ? (
        <EmptyState title="No entries" />
      ) : (
        <>
          <table className="table">
            <thead>
              <tr>
                <th>When</th>
                <th>User</th>
                <th>Action</th>
                <th>Details</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((a) => (
                <tr key={a.id}>
                  <td className="whitespace-nowrap text-slate-600">{dateTime(a.at)}</td>
                  <td>{a.username ?? 'system'}</td>
                  <td className="font-mono text-xs">{a.action}</td>
                  <td className="text-slate-800">{a.summary}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <Pager total={data.total} limit={100} offset={offset} onChange={setOffset} />
        </>
      )}
    </div>
  );
}

function About() {
  const { status, license } = useSession();
  const [pwOpen, setPwOpen] = useState(false);
  const row = (label: string, value: ReactNode) => (
    <div className="grid grid-cols-3 gap-4 border-b border-slate-100 py-2.5 text-sm last:border-0">
      <span className="text-slate-500">{label}</span>
      <span className="col-span-2 break-all text-slate-900">{value}</span>
    </div>
  );
  return (
    <div className="grid grid-cols-2 gap-5">
      <Section title="CODE X MART">
        <div className="flex items-center gap-4">
          <img src="/logo.svg" alt="" className="h-14 w-14" />
          <div>
            <p className="text-lg font-semibold text-slate-900">CODE X MART</p>
            <p className="text-sm text-slate-500">Offline retail point of sale for Windows</p>
          </div>
        </div>
        <div className="mt-5">
          {row('Version', status?.appVersion)}
          {row('Database version', status?.schemaVersion)}
          {row('Device ID', <span className="font-mono text-xs">{license?.deviceId}</span>)}
          {row('License', license?.license ? `${license.license.editionLabel} · ${license.license.serial}` : license?.headline)}
          {row('Mode', 'Offline — all data is stored on this computer')}
        </div>
        <Button className="mt-4" icon={<KeyRound className="h-4 w-4" />} onClick={() => setPwOpen(true)}>
          Change my password
        </Button>
      </Section>
      <Section title="Where your data is kept">
        {row('Data folder', <span className="font-mono text-xs">{status?.dataDir}</span>)}
        {row('Database file', <span className="font-mono text-xs">{status?.dbPath}</span>)}
        {row('Automatic backups', <span className="font-mono text-xs">{status?.backupDir}</span>)}
        {row('Log files', <span className="font-mono text-xs">{status?.logDir}</span>)}
        <div className="mt-4 flex gap-2">
          <Button size="sm" icon={<FolderOpen className="h-4 w-4" />} onClick={() => void api.app.openFolder('data')}>
            Open data folder
          </Button>
          <Button size="sm" icon={<FolderOpen className="h-4 w-4" />} onClick={() => void api.app.openFolder('logs')}>
            Open logs
          </Button>
        </div>
        <p className="mt-4 text-xs text-slate-500">
          This folder is separate from the program files, so updating or reinstalling CODE X MART keeps your data.
        </p>
      </Section>
      <ChangePasswordDialog open={pwOpen} onClose={() => setPwOpen(false)} />
    </div>
  );
}
