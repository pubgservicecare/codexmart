// Minimal localisation layer. English is the only language today; strings
// used in navigation and shared chrome go through `t()` so an Urdu
// dictionary (and `dir="rtl"`) can be added without touching components.

type Dict = Record<string, string>;

const en: Dict = {
  'nav.overview': 'Overview',
  'nav.dashboard': 'Dashboard',
  'nav.sell': 'Sell',
  'nav.pos': 'Billing',
  'nav.sales': 'Sales history',
  'nav.returns': 'Returns',
  'nav.customers': 'Customers',
  'nav.stock': 'Stock',
  'nav.products': 'Products',
  'nav.inventory': 'Inventory',
  'nav.labels': 'Barcode labels',
  'nav.buy': 'Buy',
  'nav.suppliers': 'Suppliers',
  'nav.purchases': 'Purchases',
  'nav.money': 'Money',
  'nav.expenses': 'Expenses',
  'nav.cash': 'Cash closing',
  'nav.reports': 'Reports',
  'nav.system': 'System',
  'nav.backup': 'Backup & restore',
  'nav.settings': 'Settings',
  'common.save': 'Save',
  'common.cancel': 'Cancel',
  'common.close': 'Close',
  'common.search': 'Search',
  'common.signOut': 'Sign out',
  'status.offline': 'Offline · Local data',
};

const dictionaries: Record<string, Dict> = { en };
let current: Dict = en;

export function setLanguage(lang: string) {
  current = dictionaries[lang] ?? en;
  document.documentElement.lang = lang;
  document.documentElement.dir = lang === 'ur' ? 'rtl' : 'ltr';
}

export function t(key: string): string {
  return current[key] ?? en[key] ?? key;
}
