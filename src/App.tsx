import { Component, useEffect, type ErrorInfo, type ReactNode } from 'react';
import { HashRouter, Navigate, Route, Routes, useNavigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RefreshCw, TriangleAlert } from 'lucide-react';
import { api } from '@/lib/api';
import { SessionProvider, useSession } from '@/lib/session';
import { PrintProvider } from '@/lib/print';
import { Button, ErrorBox, Spinner, ToastProvider } from '@/components/ui';
import { AppShell } from '@/components/layout/AppShell';
import { LoginScreen } from '@/features/auth/LoginScreen';
import { SetupWizard } from '@/features/auth/SetupWizard';
import { DatabaseProblem } from '@/features/auth/DatabaseProblem';
import { DashboardPage } from '@/features/dashboard/DashboardPage';
import { PosPage } from '@/features/pos/PosPage';
import { SalesPage } from '@/features/sales/SalesPage';
import { ReturnsPage } from '@/features/sales/ReturnsPage';
import { CustomersPage } from '@/features/customers/CustomersPage';
import { ProductsPage } from '@/features/products/ProductsPage';
import { InventoryPage } from '@/features/inventory/InventoryPage';
import { LabelsPage } from '@/features/inventory/LabelsPage';
import { SuppliersPage } from '@/features/purchases/SuppliersPage';
import { PurchasesPage } from '@/features/purchases/PurchasesPage';
import { ReceiveGoodsPage } from '@/features/purchases/ReceiveGoodsPage';
import { ExpensesPage } from '@/features/expenses/ExpensesPage';
import { CashPage } from '@/features/cash/CashPage';
import { ReportsPage } from '@/features/reports/ReportsPage';
import { BackupPage } from '@/features/backup/BackupPage';
import { SettingsPage } from '@/features/settings/SettingsPage';
import { LicenseScreen } from '@/features/license/LicenseScreen';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false, staleTime: 10_000 },
  },
});

class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    void api.app.logError(error.message, info.componentStack ?? undefined).catch(() => undefined);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="card max-w-md p-7 text-center">
          <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-amber-50 text-amber-600">
            <TriangleAlert className="h-6 w-6" />
          </div>
          <h2 className="text-lg font-semibold text-slate-900">This screen hit a problem</h2>
          <p className="mt-2 text-sm text-slate-600">
            No data was lost — everything saved is safe in the database. Reload the screen to continue.
          </p>
          <p className="mt-3 break-words rounded-lg bg-slate-50 px-3 py-2 font-mono text-xs text-slate-500">{this.state.error.message}</p>
          <Button variant="primary" className="mt-5" icon={<RefreshCw className="h-4 w-4" />} onClick={() => window.location.reload()}>
            Reload
          </Button>
        </div>
      </div>
    );
  }
}

/** Global keyboard shortcuts available on every screen. */
function GlobalShortcuts() {
  const navigate = useNavigate();
  const { can } = useSession();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'F1') {
        e.preventDefault();
        if (can('sell')) navigate('/pos');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [navigate, can]);
  return null;
}

function Home() {
  const { can } = useSession();
  return <Navigate to={can('viewReports') ? '/dashboard' : can('sell') ? '/pos' : '/sales'} replace />;
}

function Guard({ ok, children }: { ok: boolean; children: ReactNode }) {
  return ok ? <>{children}</> : <Navigate to="/" replace />;
}

function Root() {
  const { status, session, settings, settingsError, reloadSettings, loading, license, can } = useSession();
  if (loading) return <Spinner label="Starting CODE X MART…" />;
  if (!status) return <Spinner />;
  if (!status.dbReady) return <DatabaseProblem message={status.dbError ?? 'Unknown error'} />;
  if (status.needsSetup && !session) return <SetupWizard />;
  if (!session) return <LoginScreen />;
  // Screens read store settings (currency, receipts), so wait for them.
  if (!settings) {
    return settingsError ? <ErrorBox error={settingsError} onRetry={reloadSettings} /> : <Spinner label="Loading your store…" />;
  }
  // Free trial or license ended: activation first (shop data stays intact).
  if (license && !license.allowed) return <LicenseScreen />;

  return (
    <HashRouter>
      <GlobalShortcuts />
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<Home />} />
          <Route path="dashboard" element={<Guard ok={can('viewReports')}><DashboardPage /></Guard>} />
          <Route path="pos" element={<Guard ok={can('sell')}><PosPage /></Guard>} />
          <Route path="sales" element={<Guard ok={can('viewSales')}><SalesPage /></Guard>} />
          <Route path="returns" element={<Guard ok={can('viewSales')}><ReturnsPage /></Guard>} />
          <Route path="customers" element={<Guard ok={can('viewSales')}><CustomersPage /></Guard>} />
          <Route path="products" element={<Guard ok={can('viewCatalog')}><ProductsPage /></Guard>} />
          <Route path="inventory" element={<Guard ok={can('manageInventory')}><InventoryPage /></Guard>} />
          <Route path="labels" element={<Guard ok={can('manageCatalog')}><LabelsPage /></Guard>} />
          <Route path="suppliers" element={<Guard ok={can('managePurchasing')}><SuppliersPage /></Guard>} />
          <Route path="purchases" element={<Guard ok={can('managePurchasing')}><PurchasesPage /></Guard>} />
          <Route path="purchases/receive" element={<Guard ok={can('managePurchasing')}><ReceiveGoodsPage /></Guard>} />
          <Route path="expenses" element={<Guard ok={can('manageExpenses')}><ExpensesPage /></Guard>} />
          <Route path="cash" element={<Guard ok={can('closeCash')}><CashPage /></Guard>} />
          <Route path="reports" element={<Guard ok={can('viewReports')}><ReportsPage /></Guard>} />
          <Route path="backup" element={<Guard ok={can('createBackup')}><BackupPage /></Guard>} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Home />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}

export function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <SessionProvider>
            <PrintProvider>
              <Root />
            </PrintProvider>
          </SessionProvider>
        </ToastProvider>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}
