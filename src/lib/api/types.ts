// TypeScript mirrors of the Rust DTOs (serde camelCase).
// Money is an integer number of paisa; quantities are thousandths of a unit.

export type Money = number;
export type Qty = number;

export interface Page<T> {
  items: T[];
  total: number;
}

// ---------------------------------------------------------------- session
export type Role = 'admin' | 'manager' | 'cashier';
export type Permission =
  | 'sell'
  | 'viewSales'
  | 'voidSale'
  | 'processReturn'
  | 'viewCatalog'
  | 'manageCatalog'
  | 'manageInventory'
  | 'managePurchasing'
  | 'manageExpenses'
  | 'manageCustomers'
  | 'receiveCustomerPayment'
  | 'viewReports'
  | 'viewCosts'
  | 'closeCash'
  | 'createBackup'
  | 'restoreBackup'
  | 'manageSettings'
  | 'manageUsers'
  | 'viewAudit';

export interface Actor {
  userId: string;
  username: string;
  fullName: string;
  role: Role;
}

export interface Session {
  user: Actor;
  permissions: Permission[];
}

export interface AppStatus {
  appName: string;
  appVersion: string;
  schemaVersion: number;
  dbReady: boolean;
  dbError: string | null;
  needsSetup: boolean;
  session: Session | null;
  dataDir: string;
  dbPath: string;
  backupDir: string;
  logDir: string;
  deviceId: string | null;
}

export interface User {
  id: string;
  username: string;
  fullName: string;
  role: Role;
  isActive: boolean;
  lastLoginAt: string | null;
  createdAt: string;
}

export interface Settings {
  storeName: string;
  storeTagline: string;
  storeAddress: string;
  storePhone: string;
  storeEmail: string;
  storeTaxId: string;
  currencySymbol: string;
  currencyDecimals: number;
  receiptHeader: string;
  receiptFooter: string;
  receiptPaper: '80mm' | '58mm' | 'a4';
  receiptPrintMode: 'dialog' | 'escpos';
  receiptPrinter: string;
  receiptCopies: number;
  autoPrintReceipt: boolean;
  openCashDrawer: boolean;
  labelSize: '38x25' | '50x25' | 'a4-65';
  invoicePrefix: string;
  allowNegativeStock: boolean;
  enableCreditSales: boolean;
  enableBatchTracking: boolean;
  enableVariants: boolean;
  enableWholesale: boolean;
  maxCashierDiscountBp: number;
  expiryWarningDays: number;
  backupAutoEnabled: boolean;
  backupIntervalHours: number;
  backupRetention: number;
  backupOnExit: boolean;
  backupSecondaryDir: string;
}

export interface AuditEntry {
  id: number;
  at: string;
  username: string | null;
  action: string;
  entityType: string | null;
  entityId: string | null;
  summary: string;
  details: string | null;
}

// ---------------------------------------------------------------- catalog
export type LookupKind = 'category' | 'brand' | 'expenseCategory';

export interface LookupItem {
  id: string;
  name: string;
  isActive: boolean;
  usageCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface Unit {
  id: string;
  name: string;
  shortName: string;
  allowDecimal: boolean;
  isActive: boolean;
  usageCount: number;
}

export interface Product {
  id: string;
  name: string;
  displayName: string;
  sku: string;
  barcode: string | null;
  categoryId: string | null;
  categoryName: string | null;
  brandId: string | null;
  brandName: string | null;
  unitId: string;
  unitName: string;
  unitShort: string;
  allowDecimal: boolean;
  variantGroupId: string | null;
  size: string | null;
  color: string | null;
  description: string | null;
  salePrice: Money;
  wholesalePrice: Money | null;
  costPrice: Money | null;
  stockQty: Qty;
  stockValue: Money | null;
  reorderLevel: Qty;
  trackBatches: boolean;
  imageRef: string | null;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ProductFilter {
  search?: string;
  categoryId?: string;
  brandId?: string;
  status?: 'active' | 'inactive' | 'all';
  stock?: '' | 'low' | 'out' | 'attention' | 'in';
  variantGroupId?: string;
  sort?: 'name' | 'stock' | 'updated' | 'sku' | 'price';
  limit?: number;
  offset?: number;
}

export interface OpeningStock {
  qty: Qty;
  unitCost?: Money | null;
  batchNo?: string | null;
  expiryDate?: string | null;
}

export interface ProductInput {
  name: string;
  sku?: string | null;
  barcode?: string | null;
  categoryId?: string | null;
  brandId?: string | null;
  unitId: string;
  variantGroupId?: string | null;
  size?: string | null;
  color?: string | null;
  description?: string | null;
  salePrice: Money;
  wholesalePrice?: Money | null;
  costPrice?: Money | null;
  reorderLevel: Qty;
  trackBatches: boolean;
  imageRef?: string | null;
  isActive?: boolean;
  openingStock?: OpeningStock | null;
}

// -------------------------------------------------------------- inventory
export interface Movement {
  id: string;
  productId: string;
  productName: string;
  sku: string;
  unitShort: string;
  batchId: string | null;
  batchNo: string | null;
  movementType: string;
  qtyChange: Qty;
  valueChange: Money | null;
  qtyAfter: Qty;
  refType: string | null;
  refId: string | null;
  refNo: string | null;
  reason: string | null;
  note: string | null;
  userName: string | null;
  businessDate: string;
  createdAt: string;
}

export interface MovementFilter {
  productId?: string;
  movementType?: string;
  dateFrom?: string;
  dateTo?: string;
  limit?: number;
  offset?: number;
}

export interface Batch {
  id: string;
  productId: string;
  productName: string;
  sku: string;
  unitShort: string;
  batchNo: string | null;
  expiryDate: string | null;
  qtyReceived: Qty;
  qtyRemaining: Qty;
  unitCost: Money | null;
  receivedDate: string;
  daysToExpiry: number | null;
}

export interface BatchFilter {
  productId?: string;
  onlyAvailable?: boolean;
  expiringWithinDays?: number;
}

export type AdjustReason =
  | 'opening_stock'
  | 'stock_count'
  | 'found'
  | 'damaged'
  | 'expired'
  | 'lost'
  | 'theft'
  | 'internal_use'
  | 'other';

export interface AdjustmentLineInput {
  productId: string;
  mode: 'add' | 'remove' | 'set';
  qty: Qty;
  unitCost?: Money | null;
  batchId?: string | null;
  batchNo?: string | null;
  expiryDate?: string | null;
}

export interface AdjustmentInput {
  clientTxnId: string;
  reason: AdjustReason;
  note?: string | null;
  lines: AdjustmentLineInput[];
}

export interface Adjustment {
  id: string;
  adjustmentNo: string;
  reason: AdjustReason;
  note: string | null;
  valueChange: Money | null;
  userName: string | null;
  businessDate: string;
  createdAt: string;
  lines: {
    productId: string;
    productName: string;
    sku: string;
    unitShort: string;
    batchNo: string | null;
    qtyChange: Qty;
    valueChange: Money | null;
  }[];
}

export interface DateFilter {
  dateFrom?: string;
  dateTo?: string;
  limit?: number;
  offset?: number;
}

export interface LedgerMismatch {
  productId: string;
  productName: string;
  stockQty: Qty;
  ledgerQty: Qty;
  stockValue: Money;
  ledgerValue: Money;
}

// ------------------------------------------------------------------ sales
export type PaymentMethod = 'cash' | 'card' | 'bank' | 'wallet' | 'cheque';
export type RefundMethod = PaymentMethod | 'account';

export interface Discount {
  kind: 'amount' | 'percent';
  /** Paisa for amounts, basis points for percent (1250 = 12.5%). */
  value: number;
}

export interface CartLine {
  productId: string;
  qty: Qty;
  discount?: Discount | null;
}

export interface Cart {
  lines: CartLine[];
  billDiscount?: Discount | null;
  priceTier?: 'retail' | 'wholesale';
  customerId?: string | null;
}

export interface CartTotals {
  subtotal: Money;
  lineDiscountTotal: Money;
  billDiscount: Money;
  discountTotal: Money;
  total: Money;
}

export interface QuoteLine {
  productId: string;
  name: string;
  sku: string;
  barcode: string | null;
  unitShort: string;
  allowDecimal: boolean;
  qty: Qty;
  unitPrice: Money;
  gross: Money;
  lineDiscount: Money;
  billDiscountShare: Money;
  net: Money;
  stockQty: Qty;
  stockOk: boolean;
}

export interface CustomerBrief {
  id: string;
  name: string;
  phone: string | null;
  balance: Money;
  creditLimit: Money | null;
}

export interface Quote {
  lines: QuoteLine[];
  totals: CartTotals;
  itemCount: number;
  totalQty: Qty;
  customer: CustomerBrief | null;
  warnings: string[];
}

export interface Tender {
  method: PaymentMethod;
  amount: Money;
  reference?: string | null;
}

export interface CheckoutInput {
  clientTxnId: string;
  cart: Cart;
  payments: Tender[];
  note?: string | null;
}

export interface SaleSummary {
  id: string;
  invoiceNo: string;
  status: 'completed' | 'void';
  customerId: string | null;
  customerName: string | null;
  itemCount: number;
  totalQty: Qty;
  subtotal: Money;
  discountTotal: Money;
  total: Money;
  paidAmount: Money;
  creditAmount: Money;
  returnedAmount: Money;
  cashierName: string;
  businessDate: string;
  createdAt: string;
}

export interface SaleItem {
  id: string;
  lineNo: number;
  productId: string;
  productName: string;
  sku: string;
  barcode: string | null;
  unitName: string;
  qty: Qty;
  unitPrice: Money;
  grossAmount: Money;
  lineDiscount: Money;
  billDiscountShare: Money;
  netAmount: Money;
  costAmount: Money | null;
  returnedQty: Qty;
}

export interface SaleReturnSummary {
  id: string;
  returnNo: string;
  saleId: string;
  invoiceNo: string;
  customerName: string | null;
  refundMethod: RefundMethod;
  totalRefund: Money;
  reason: string | null;
  userName: string;
  businessDate: string;
  createdAt: string;
}

export interface SaleDetail extends SaleSummary {
  priceTier: string;
  lineDiscountTotal: Money;
  billDiscount: Money;
  tenderedCash: Money;
  changeAmount: Money;
  cogsTotal: Money | null;
  note: string | null;
  customerPhone: string | null;
  voidedAt: string | null;
  voidReason: string | null;
  voidedByName: string | null;
  items: SaleItem[];
  payments: { method: PaymentMethod; amount: Money; reference: string | null }[];
  returns: SaleReturnSummary[];
}

export interface SaleFilter {
  search?: string;
  status?: string;
  customerId?: string;
  dateFrom?: string;
  dateTo?: string;
  limit?: number;
  offset?: number;
}

export interface ReturnableLine {
  saleItemId: string;
  productId: string;
  productName: string;
  sku: string;
  unitName: string;
  allowDecimal: boolean;
  qtySold: Qty;
  qtyReturned: Qty;
  qtyReturnable: Qty;
  unitPrice: Money;
  netAmount: Money;
  refundableAmount: Money;
}

export interface ReturnInput {
  clientTxnId: string;
  saleId: string;
  lines: { saleItemId: string; qty: Qty; restock: boolean }[];
  refundMethod: RefundMethod;
  reason?: string | null;
}

export interface SaleReturnDetail extends SaleReturnSummary {
  lines: { saleItemId: string; productName: string; sku: string; qty: Qty; refundAmount: Money; restock: boolean }[];
}

// -------------------------------------------------------------- customers
export interface Customer {
  id: string;
  name: string;
  phone: string | null;
  email: string | null;
  address: string | null;
  openingBalance: Money;
  creditLimit: Money | null;
  notes: string | null;
  isActive: boolean;
  balance: Money;
  totalPurchases: Money;
  lastPurchaseDate: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CustomerInput {
  name: string;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  openingBalance: Money;
  creditLimit?: Money | null;
  notes?: string | null;
  isActive?: boolean;
}

export interface CustomerFilter {
  search?: string;
  status?: string;
  withBalance?: boolean;
  limit?: number;
  offset?: number;
}

export interface LedgerEntry {
  date: string;
  createdAt: string;
  kind: string;
  reference: string;
  refId: string | null;
  description: string;
  debit: Money;
  credit: Money;
  balance: Money;
}

export interface CustomerPayment {
  id: string;
  receiptNo: string;
  customerId: string;
  customerName: string;
  amount: Money;
  method: PaymentMethod;
  note: string | null;
  userName: string;
  businessDate: string;
  createdAt: string;
}

// ------------------------------------------------------------- purchasing
export interface Supplier {
  id: string;
  name: string;
  contactPerson: string | null;
  phone: string | null;
  email: string | null;
  address: string | null;
  openingBalance: Money;
  notes: string | null;
  isActive: boolean;
  totalPurchases: Money;
  totalPayments: Money;
  totalReturns: Money;
  balance: Money;
  lastPurchaseDate: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SupplierInput {
  name: string;
  contactPerson?: string | null;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  openingBalance: Money;
  notes?: string | null;
  isActive?: boolean;
}

export interface SupplierLedgerEntry {
  date: string;
  createdAt: string;
  kind: string;
  reference: string;
  refId: string | null;
  description: string;
  credit: Money;
  debit: Money;
  balance: Money;
}

export interface DocFilter {
  supplierId?: string;
  status?: string;
  search?: string;
  dateFrom?: string;
  dateTo?: string;
  limit?: number;
  offset?: number;
}

export interface PurchaseSummary {
  id: string;
  purchaseNo: string;
  supplierId: string;
  supplierName: string;
  supplierInvoiceNo: string | null;
  poId: string | null;
  poNo: string | null;
  itemCount: number;
  subtotal: Money;
  discount: Money;
  total: Money;
  paidAmount: Money;
  returnedAmount: Money;
  note: string | null;
  userName: string;
  businessDate: string;
  createdAt: string;
}

export interface PurchaseItem {
  id: string;
  lineNo: number;
  productId: string;
  productName: string;
  sku: string;
  unitShort: string;
  qty: Qty;
  unitCost: Money;
  lineTotal: Money;
  discountShare: Money;
  netCost: Money;
  batchId: string | null;
  batchNo: string | null;
  expiryDate: string | null;
  returnedQty: Qty;
}

export interface PurchaseDetail extends PurchaseSummary {
  items: PurchaseItem[];
}

export interface PurchaseLineInput {
  productId: string;
  qty: Qty;
  unitCost: Money;
  batchNo?: string | null;
  expiryDate?: string | null;
  poItemId?: string | null;
  newSalePrice?: Money | null;
}

export interface PurchaseInput {
  clientTxnId: string;
  supplierId: string;
  supplierInvoiceNo?: string | null;
  poId?: string | null;
  businessDate?: string | null;
  discount: Money;
  note?: string | null;
  lines: PurchaseLineInput[];
  payment?: { amount: Money; method: PaymentMethod; reference?: string | null } | null;
}

export interface PoItem {
  id: string;
  lineNo: number;
  productId: string;
  productName: string;
  sku: string;
  unitShort: string;
  allowDecimal: boolean;
  qtyOrdered: Qty;
  qtyReceived: Qty;
  unitCost: Money;
}

export interface PurchaseOrder {
  id: string;
  poNo: string;
  supplierId: string;
  supplierName: string;
  status: 'draft' | 'ordered' | 'partial' | 'received' | 'cancelled';
  orderDate: string;
  expectedDate: string | null;
  note: string | null;
  totalEstimate: Money;
  itemCount: number;
  createdByName: string;
  createdAt: string;
  updatedAt: string;
  items: PoItem[];
}

export interface PoInput {
  supplierId: string;
  orderDate?: string | null;
  expectedDate?: string | null;
  note?: string | null;
  status?: 'draft' | 'ordered';
  lines: { productId: string; qty: Qty; unitCost: Money }[];
}

export interface SupplierPayment {
  id: string;
  paymentNo: string;
  supplierId: string;
  supplierName: string;
  purchaseId: string | null;
  purchaseNo: string | null;
  amount: Money;
  method: PaymentMethod;
  reference: string | null;
  note: string | null;
  userName: string;
  businessDate: string;
  createdAt: string;
}

export interface SupplierPaymentInput {
  clientTxnId: string;
  supplierId: string;
  purchaseId?: string | null;
  amount: Money;
  method: PaymentMethod;
  reference?: string | null;
  note?: string | null;
  businessDate?: string | null;
}

export interface PurchaseReturn {
  id: string;
  returnNo: string;
  supplierId: string;
  supplierName: string;
  purchaseId: string;
  purchaseNo: string;
  total: Money;
  reason: string | null;
  userName: string;
  businessDate: string;
  createdAt: string;
  lines: { productName: string; sku: string; unitShort: string; qty: Qty; lineTotal: Money }[];
}

export interface PurchaseReturnInput {
  clientTxnId: string;
  purchaseId: string;
  lines: { purchaseItemId: string; qty: Qty }[];
  reason?: string | null;
}

// --------------------------------------------------------------- expenses
export interface Expense {
  id: string;
  expenseNo: string;
  categoryId: string;
  categoryName: string;
  amount: Money;
  method: PaymentMethod;
  payee: string | null;
  note: string | null;
  businessDate: string;
  status: 'active' | 'void';
  voidReason: string | null;
  userName: string;
  createdAt: string;
}

export interface ExpenseInput {
  clientTxnId: string;
  categoryId: string;
  amount: Money;
  method: PaymentMethod;
  payee?: string | null;
  note?: string | null;
  businessDate?: string | null;
}

export interface ExpenseFilter extends DateFilter {
  categoryId?: string;
  status?: string;
  search?: string;
}

// ------------------------------------------------------------------- cash
export interface CashClosing {
  id: string;
  closingNo: string;
  businessDate: string;
  openingFloat: Money;
  cashSales: Money;
  cashCustomerReceipts: Money;
  cashRefunds: Money;
  cashExpenses: Money;
  cashSupplierPayments: Money;
  expectedCash: Money;
  countedCash: Money;
  difference: Money;
  note: string | null;
  userName: string;
  createdAt: string;
}

export interface CashDay {
  businessDate: string;
  invoices: number;
  cashSales: Money;
  cashCustomerReceipts: Money;
  cashRefunds: Money;
  cashExpenses: Money;
  cashSupplierPayments: Money;
  netCash: Money;
  salesByMethod: { method: string; amount: Money }[];
  closing: CashClosing | null;
}

// ---------------------------------------------------------------- reports
export interface DateRange {
  from: string;
  to: string;
}

export interface NamedAmount {
  name: string;
  amount: Money;
  count: number;
}

export interface ProfitLoss {
  from: string;
  to: string;
  invoices: number;
  grossSales: Money;
  discounts: Money;
  salesTotal: Money;
  returns: Money;
  netSales: Money;
  cogsSold: Money;
  cogsReturned: Money;
  cogs: Money;
  grossProfit: Money;
  grossMarginBp: number;
  expenses: NamedAmount[];
  expensesTotal: Money;
  inventoryAdjustments: Money;
  netOperatingResult: Money;
  averageBill: Money;
}

export interface SalesPeriodRow {
  period: string;
  invoices: number;
  grossSales: Money;
  discounts: Money;
  salesTotal: Money;
  returnsTotal: Money;
  netSales: Money;
  cogs: Money;
  grossProfit: Money;
  marginBp: number;
}

export interface SalesSummary {
  groupBy: string;
  rows: SalesPeriodRow[];
  totals: SalesPeriodRow;
}

export interface ProductSalesRow {
  productId: string;
  name: string;
  sku: string;
  category: string;
  brand: string;
  qtySold: Qty;
  qtyReturned: Qty;
  salesAmount: Money;
  returnsAmount: Money;
  netSales: Money;
  cogs: Money;
  grossProfit: Money;
  marginBp: number;
}

export interface CategorySalesRow {
  category: string;
  products: number;
  qtySold: Qty;
  netSales: Money;
  cogs: Money;
  grossProfit: Money;
  marginBp: number;
  shareBp: number;
}

export interface PurchaseReport {
  bySupplier: { supplierId: string; supplierName: string; invoices: number; purchases: Money; returns: Money; payments: Money }[];
  invoices: number;
  purchasesTotal: Money;
  discounts: Money;
  returnsTotal: Money;
  netPurchases: Money;
  paymentsTotal: Money;
  documents: PurchaseSummary[];
}

export interface ExpenseReport {
  byCategory: NamedAmount[];
  byMethod: NamedAmount[];
  total: Money;
  count: number;
}

export interface ReturnsReport {
  count: number;
  totalRefund: Money;
  byMethod: NamedAmount[];
  topProducts: ProductSalesRow[];
  returns: SaleReturnSummary[];
}

export interface ValuationRow {
  productId: string;
  name: string;
  sku: string;
  category: string;
  unitShort: string;
  qty: Qty;
  unitCost: Money;
  costValue: Money;
  salePrice: Money;
  retailValue: Money;
}

export interface InventoryValuation {
  rows: ValuationRow[];
  byCategory: NamedAmount[];
  totalCostValue: Money;
  totalRetailValue: Money;
  totalQty: Qty;
  productsInStock: number;
}

export interface TrendPoint {
  date: string;
  netSales: Money;
  grossProfit: Money;
  invoices: number;
}

export interface Dashboard {
  today: string;
  day: ProfitLoss;
  month: ProfitLoss;
  stockValue: Money;
  stockRetailValue: Money;
  productsInStock: number;
  lowStockCount: number;
  outOfStockCount: number;
  lowStock: Product[];
  recentSales: SaleSummary[];
  trend: TrendPoint[];
  purchasesMonth: Money;
  payables: Money;
  receivables: Money;
  expiringSoon: number;
}

// ----------------------------------------------------------------- backup
export interface BackupManifest {
  format: string;
  formatVersion: number;
  appName: string;
  appVersion: string;
  schemaVersion: number;
  createdAt: string;
  kind: string;
  dbSha256: string;
  dbSizeBytes: number;
  dbId: string | null;
  deviceId: string | null;
  storeName: string | null;
  counts: Record<string, number>;
}

export interface BackupFile {
  path: string;
  fileName: string;
  kind: string;
  sizeBytes: number;
  modifiedAt: string;
}

export interface BackupHistoryEntry {
  at: string;
  action: string;
  kind: string;
  status: 'success' | 'failed';
  path: string | null;
  sizeBytes: number | null;
  message: string;
}

export interface BackupStatus {
  backupDir: string;
  secondaryDir: string;
  secondaryAvailable: boolean;
  autoEnabled: boolean;
  intervalHours: number;
  retention: number;
  lastSuccessAt: string | null;
  sameDriveOnly: boolean;
  history: BackupHistoryEntry[];
  files: BackupFile[];
}

export interface BackupRun {
  path: string;
  fileName: string;
  sizeBytes: number;
  manifest: BackupManifest;
  secondary: string | null;
}

export interface BackupCheck {
  valid: boolean;
  message: string;
  manifest: BackupManifest | null;
  fileSize: number;
}

export interface RestoreReport {
  restoredFrom: string;
  safetyBackup: string | null;
  backupCreatedAt: string;
}

export interface PrinterInfo {
  name: string;
  isDefault: boolean;
}

export interface HealthReport {
  integrityProblems: string[];
  stockMismatches: number;
  schemaVersion: number;
  dbSizeBytes: number;
}
