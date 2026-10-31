import { call } from './client';
import type * as T from './types';

export * from './types';
export { ApiError, errorMessage, onApiError } from './client';

/** Typed access to every backend command, grouped by module. */
export const api = {
  app: {
    status: () => call<T.AppStatus>('app_status'),
    retryOpen: () => call<void>('app_retry_open'),
    setup: (input: {
      storeName: string;
      storePhone?: string;
      storeAddress?: string;
      currencySymbol?: string;
      adminFullName: string;
      adminUsername: string;
      adminPassword: string;
    }) => call<{ session: T.Session; recoveryKey: string }>('app_setup', { input }),
    health: () => call<T.HealthReport>('app_health_check'),
    openFolder: (kind: 'data' | 'backups' | 'logs' | 'custom', path?: string) =>
      call<void>('app_open_folder', { kind, path }),
    exportText: (path: string, contents: string) => call<void>('export_text_file', { path, contents }),
    logError: (message: string, detail?: string) => call<void>('log_client_error', { message, detail }),
  },
  auth: {
    login: (username: string, password: string) => call<T.Session>('auth_login', { username, password }),
    logout: () => call<void>('auth_logout'),
    changePassword: (current: string, newPassword: string) =>
      call<void>('auth_change_password', { current, newPassword }),
    recover: (recoveryKey: string, username: string, newPassword: string) =>
      call<void>('auth_recover', { recoveryKey, username, newPassword }),
    regenerateRecoveryKey: () => call<string>('auth_regenerate_recovery_key'),
  },
  users: {
    list: () => call<T.User[]>('users_list'),
    create: (input: { username: string; fullName: string; role: T.Role; password: string }) =>
      call<T.User>('users_create', { input }),
    update: (id: string, input: { fullName: string; role: T.Role; isActive: boolean }) =>
      call<T.User>('users_update', { id, input }),
    resetPassword: (id: string, newPassword: string) => call<void>('users_reset_password', { id, newPassword }),
  },
  settings: {
    get: () => call<T.Settings>('settings_get'),
    save: (input: T.Settings) => call<T.Settings>('settings_save', { input }),
  },
  audit: {
    list: (filter: { search?: string; entityType?: string; dateFrom?: string; dateTo?: string; limit?: number; offset?: number }) =>
      call<T.Page<T.AuditEntry>>('audit_list', { filter }),
  },
  lookups: {
    list: (kind: T.LookupKind, includeInactive = false) => call<T.LookupItem[]>('lookups_list', { kind, includeInactive }),
    create: (kind: T.LookupKind, name: string) => call<T.LookupItem>('lookups_create', { kind, name }),
    update: (kind: T.LookupKind, id: string, name: string, isActive: boolean) =>
      call<T.LookupItem>('lookups_update', { kind, id, name, isActive }),
  },
  units: {
    list: (includeInactive = false) => call<T.Unit[]>('units_list', { includeInactive }),
    create: (input: { name: string; shortName: string; allowDecimal: boolean; isActive?: boolean }) =>
      call<T.Unit>('units_create', { input }),
    update: (id: string, input: { name: string; shortName: string; allowDecimal: boolean; isActive: boolean }) =>
      call<T.Unit>('units_update', { id, input }),
  },
  products: {
    list: (filter: T.ProductFilter) => call<T.Page<T.Product>>('products_list', { filter }),
    get: (id: string) => call<T.Product>('products_get', { id }),
    search: (query: string, limit = 30) => call<T.Product[]>('products_search', { query, limit }),
    findByCode: (code: string) => call<T.Product | null>('products_find_by_code', { code }),
    create: (input: T.ProductInput) => call<T.Product>('products_create', { input }),
    update: (id: string, input: T.ProductInput) => call<T.Product>('products_update', { id, input }),
    setActive: (id: string, active: boolean) => call<T.Product>('products_set_active', { id, active }),
    generateBarcode: () => call<string>('products_generate_barcode'),
  },
  inventory: {
    adjust: (input: T.AdjustmentInput) => call<T.Adjustment>('inventory_adjust', { input }),
    adjustments: (filter: T.DateFilter & { reason?: string }) =>
      call<T.Page<T.Adjustment>>('inventory_adjustments', { filter }),
    movements: (filter: T.MovementFilter) => call<T.Page<T.Movement>>('inventory_movements', { filter }),
    batches: (filter: T.BatchFilter) => call<T.Batch[]>('inventory_batches', { filter }),
    verify: () => call<T.LedgerMismatch[]>('inventory_verify'),
  },
  sales: {
    quote: (cart: T.Cart) => call<T.Quote>('sales_quote', { cart }),
    checkout: (input: T.CheckoutInput) => call<T.SaleDetail>('sales_checkout', { input }),
    list: (filter: T.SaleFilter) => call<T.Page<T.SaleSummary>>('sales_list', { filter }),
    get: (id: string) => call<T.SaleDetail>('sales_get', { id }),
    void: (id: string, reason: string) => call<T.SaleDetail>('sales_void', { id, reason }),
    returnable: (saleId: string) => call<T.ReturnableLine[]>('sales_returnable', { saleId }),
    createReturn: (input: T.ReturnInput) => call<T.SaleReturnDetail>('sales_return', { input }),
    returns: (filter: T.DateFilter & { saleId?: string; search?: string }) =>
      call<T.Page<T.SaleReturnSummary>>('sales_returns_list', { filter }),
    getReturn: (id: string) => call<T.SaleReturnDetail>('sales_return_get', { id }),
  },
  customers: {
    list: (filter: T.CustomerFilter) => call<T.Page<T.Customer>>('customers_list', { filter }),
    get: (id: string) => call<T.Customer>('customers_get', { id }),
    create: (input: T.CustomerInput) => call<T.Customer>('customers_create', { input }),
    update: (id: string, input: T.CustomerInput) => call<T.Customer>('customers_update', { id, input }),
    ledger: (id: string) => call<T.LedgerEntry[]>('customers_ledger', { id }),
    receivePayment: (input: { clientTxnId: string; customerId: string; amount: T.Money; method: T.PaymentMethod; note?: string }) =>
      call<T.CustomerPayment>('customers_receive_payment', { input }),
    payments: (filter: T.DateFilter & { customerId?: string }) =>
      call<T.Page<T.CustomerPayment>>('customer_payments_list', { filter }),
  },
  cash: {
    preview: (date?: string) => call<T.CashDay>('cash_preview', { date }),
    close: (input: { businessDate?: string; openingFloat: T.Money; countedCash: T.Money; note?: string }) =>
      call<T.CashClosing>('cash_close', { input }),
    closings: (from: string, to: string) => call<T.CashClosing[]>('cash_closings', { from, to }),
  },
  printing: {
    printers: () => call<T.PrinterInfo[]>('printers_list'),
    receiptDirect: (saleId: string, reprint: boolean) => call<void>('print_receipt_direct', { saleId, reprint }),
    testDirect: (printer: string, paper: string) => call<void>('print_test_direct', { printer, paper }),
  },
  suppliers: {
    list: (filter: { search?: string; status?: string; limit?: number }) =>
      call<T.Page<T.Supplier>>('suppliers_list', { filter }),
    get: (id: string) => call<T.Supplier>('suppliers_get', { id }),
    create: (input: T.SupplierInput) => call<T.Supplier>('suppliers_create', { input }),
    update: (id: string, input: T.SupplierInput) => call<T.Supplier>('suppliers_update', { id, input }),
    ledger: (id: string) => call<T.SupplierLedgerEntry[]>('suppliers_ledger', { id }),
  },
  purchases: {
    list: (filter: T.DocFilter) => call<T.Page<T.PurchaseSummary>>('purchases_list', { filter }),
    get: (id: string) => call<T.PurchaseDetail>('purchases_get', { id }),
    create: (input: T.PurchaseInput) => call<T.PurchaseDetail>('purchases_create', { input }),
    orders: (filter: T.DocFilter) => call<T.Page<T.PurchaseOrder>>('po_list', { filter }),
    getOrder: (id: string) => call<T.PurchaseOrder>('po_get', { id }),
    createOrder: (input: T.PoInput) => call<T.PurchaseOrder>('po_create', { input }),
    updateOrder: (id: string, input: T.PoInput) => call<T.PurchaseOrder>('po_update', { id, input }),
    setOrderStatus: (id: string, action: 'ordered' | 'cancelled' | 'closed') =>
      call<T.PurchaseOrder>('po_set_status', { id, action }),
    payments: (filter: T.DocFilter) => call<T.Page<T.SupplierPayment>>('supplier_payments_list', { filter }),
    pay: (input: T.SupplierPaymentInput) => call<T.SupplierPayment>('supplier_payments_create', { input }),
    returns: (filter: T.DocFilter) => call<T.Page<T.PurchaseReturn>>('purchase_returns_list', { filter }),
    createReturn: (input: T.PurchaseReturnInput) => call<T.PurchaseReturn>('purchase_returns_create', { input }),
  },
  expenses: {
    list: (filter: T.ExpenseFilter) => call<T.Page<T.Expense>>('expenses_list', { filter }),
    create: (input: T.ExpenseInput) => call<T.Expense>('expenses_create', { input }),
    void: (id: string, reason: string) => call<T.Expense>('expenses_void', { id, reason }),
  },
  reports: {
    dashboard: () => call<T.Dashboard>('dashboard_get'),
    salesSummary: (range: T.DateRange, groupBy: 'day' | 'week' | 'month') =>
      call<T.SalesSummary>('reports_sales_summary', { range, groupBy }),
    profitLoss: (range: T.DateRange) => call<T.ProfitLoss>('reports_profit_loss', { range }),
    productSales: (range: T.DateRange) => call<T.ProductSalesRow[]>('reports_product_sales', { range }),
    categorySales: (range: T.DateRange) => call<T.CategorySalesRow[]>('reports_category_sales', { range }),
    purchases: (range: T.DateRange) => call<T.PurchaseReport>('reports_purchases', { range }),
    expenses: (range: T.DateRange) => call<T.ExpenseReport>('reports_expenses', { range }),
    returns: (range: T.DateRange) => call<T.ReturnsReport>('reports_returns', { range }),
    paymentMethods: (range: T.DateRange) => call<T.NamedAmount[]>('reports_payment_methods', { range }),
    supplierBalances: () => call<T.Supplier[]>('reports_supplier_balances'),
    customerBalances: () => call<T.Customer[]>('reports_customer_balances'),
    inventoryValuation: () => call<T.InventoryValuation>('reports_inventory_valuation'),
    lowStock: () => call<T.Product[]>('reports_low_stock'),
    expiring: (days: number) => call<T.Batch[]>('reports_expiring', { days }),
  },
  backup: {
    status: () => call<T.BackupStatus>('backup_status'),
    create: (destDir?: string) => call<T.BackupRun>('backup_create', { destDir }),
    listFolder: (dir: string) => call<T.BackupFile[]>('backup_list_folder', { dir }),
    validate: (path: string) => call<T.BackupCheck>('backup_validate', { path }),
    restore: (path: string, confirm: string) => call<T.RestoreReport>('backup_restore', { path, confirm }),
  },
};
