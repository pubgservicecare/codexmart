import { useEffect, useState, type ReactNode } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router-dom';
import {
  Barcode,
  Boxes,
  Building,
  Calculator,
  ChartColumn,
  ClipboardList,
  DatabaseBackup,
  KeyRound,
  LayoutDashboard,
  LogOut,
  Package,
  Receipt,
  Settings,
  ShoppingCart,
  Undo2,
  Users,
  Wallet,
  WifiOff,
} from 'lucide-react';
import { useSession } from '@/lib/session';
import { t } from '@/lib/i18n';
import type { Permission } from '@/lib/api';
import { cx } from '@/lib/utils';
import { ChangePasswordDialog } from '@/features/auth/ChangePasswordDialog';
import { LicenseReminder } from '@/features/license/LicenseReminder';

interface NavItem {
  to: string;
  label: string;
  icon: ReactNode;
  perm: Permission;
  shortcut?: string;
}

const SECTIONS: { title: string; items: NavItem[] }[] = [
  {
    title: 'nav.overview',
    items: [{ to: '/dashboard', label: 'nav.dashboard', icon: <LayoutDashboard />, perm: 'viewReports' }],
  },
  {
    title: 'nav.sell',
    items: [
      { to: '/pos', label: 'nav.pos', icon: <ShoppingCart />, perm: 'sell', shortcut: 'F1' },
      { to: '/sales', label: 'nav.sales', icon: <Receipt />, perm: 'viewSales' },
      { to: '/returns', label: 'nav.returns', icon: <Undo2 />, perm: 'viewSales' },
      { to: '/customers', label: 'nav.customers', icon: <Users />, perm: 'viewSales' },
    ],
  },
  {
    title: 'nav.stock',
    items: [
      { to: '/products', label: 'nav.products', icon: <Package />, perm: 'viewCatalog' },
      { to: '/inventory', label: 'nav.inventory', icon: <Boxes />, perm: 'manageInventory' },
      { to: '/labels', label: 'nav.labels', icon: <Barcode />, perm: 'manageCatalog' },
    ],
  },
  {
    title: 'nav.buy',
    items: [
      { to: '/suppliers', label: 'nav.suppliers', icon: <Building />, perm: 'managePurchasing' },
      { to: '/purchases', label: 'nav.purchases', icon: <ClipboardList />, perm: 'managePurchasing' },
    ],
  },
  {
    title: 'nav.money',
    items: [
      { to: '/expenses', label: 'nav.expenses', icon: <Wallet />, perm: 'manageExpenses' },
      { to: '/cash', label: 'nav.cash', icon: <Calculator />, perm: 'closeCash' },
      { to: '/reports', label: 'nav.reports', icon: <ChartColumn />, perm: 'viewReports' },
    ],
  },
  {
    title: 'nav.system',
    items: [
      { to: '/backup', label: 'nav.backup', icon: <DatabaseBackup />, perm: 'createBackup' },
      { to: '/settings', label: 'nav.settings', icon: <Settings />, perm: 'viewSales' },
    ],
  },
];

function Clock() {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 20_000);
    return () => clearInterval(id);
  }, []);
  return (
    <span className="num">
      {now.toLocaleDateString('en-GB', { weekday: 'short', day: '2-digit', month: 'short' })} ·{' '}
      {now.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })}
    </span>
  );
}

export function AppShell() {
  const { session, settings, can, signOut } = useSession();
  const location = useLocation();
  const [pwOpen, setPwOpen] = useState(false);
  const isPos = location.pathname.startsWith('/pos');

  return (
    <div className="flex h-full">
      <aside className="flex w-60 shrink-0 flex-col bg-ink-950 text-slate-300">
        <div className="flex items-center gap-2.5 px-5 pb-4 pt-4">
          <img src="/logo.svg" alt="" className="h-9 w-9" />
          <div className="leading-tight">
            <p className="text-[15px] font-bold tracking-tight text-white">CODE X MART</p>
            <p className="truncate text-2xs text-slate-400">{settings?.storeName}</p>
          </div>
        </div>
        <nav className="flex-1 space-y-4 overflow-y-auto px-3 pb-3">
          {SECTIONS.map((section) => {
            const items = section.items.filter((i) => can(i.perm));
            if (!items.length) return null;
            return (
              <div key={section.title}>
                <p className="mb-1.5 px-2.5 text-2xs font-semibold uppercase tracking-wider text-slate-500">{t(section.title)}</p>
                <div className="space-y-0.5">
                  {items.map((item) => (
                    <NavLink
                      key={item.to}
                      to={item.to}
                      className={({ isActive }) =>
                        cx(
                          'group flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[13px] font-medium transition [&>svg]:h-[18px] [&>svg]:w-[18px]',
                          isActive ? 'bg-white/10 text-white shadow-inner' : 'text-slate-400 hover:bg-white/5 hover:text-slate-100',
                        )
                      }
                    >
                      {item.icon}
                      <span className="flex-1">{t(item.label)}</span>
                      {item.shortcut && <span className="text-2xs text-slate-600 group-hover:text-slate-400">{item.shortcut}</span>}
                    </NavLink>
                  ))}
                </div>
              </div>
            );
          })}
        </nav>
        <div className="border-t border-white/5 p-3">
          <div className="flex items-center gap-2.5 rounded-lg px-2 py-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-600 text-xs font-bold uppercase text-white">
              {session?.user.fullName.slice(0, 2)}
            </div>
            <div className="min-w-0 flex-1 leading-tight">
              <p className="truncate text-[13px] font-medium text-slate-100">{session?.user.fullName}</p>
              <p className="text-2xs capitalize text-slate-500">{session?.user.role}</p>
            </div>
            <button
              className="rounded-md p-1.5 text-slate-500 hover:bg-white/5 hover:text-slate-200"
              title="Change password"
              onClick={() => setPwOpen(true)}
            >
              <KeyRound className="h-4 w-4" />
            </button>
            <button className="rounded-md p-1.5 text-slate-500 hover:bg-white/5 hover:text-slate-200" title={t('common.signOut')} onClick={signOut}>
              <LogOut className="h-4 w-4" />
            </button>
          </div>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-12 shrink-0 items-center justify-end gap-4 border-b border-slate-200/80 bg-white/70 px-6 text-xs text-slate-500 backdrop-blur">
          <LicenseReminder />
          <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-1 font-medium text-emerald-700 ring-1 ring-inset ring-emerald-200">
            <WifiOff className="h-3.5 w-3.5" />
            {t('status.offline')}
          </span>
          <Clock />
        </header>
        <main className={cx('min-h-0 flex-1', isPos ? 'overflow-hidden' : 'overflow-y-auto px-6 py-6')}>
          <Outlet />
        </main>
      </div>
      <ChangePasswordDialog open={pwOpen} onClose={() => setPwOpen(false)} />
    </div>
  );
}
