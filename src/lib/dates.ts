import { addDays, toYmd, today } from './format';
import type { DateRange } from './api';

export type PresetKey = 'today' | 'yesterday' | 'last7' | 'thisMonth' | 'lastMonth' | 'last30' | 'thisYear';

export const PRESETS: { key: PresetKey; label: string }[] = [
  { key: 'today', label: 'Today' },
  { key: 'yesterday', label: 'Yesterday' },
  { key: 'last7', label: 'Last 7 days' },
  { key: 'thisMonth', label: 'This month' },
  { key: 'lastMonth', label: 'Last month' },
  { key: 'last30', label: 'Last 30 days' },
  { key: 'thisYear', label: 'This year' },
];

export function presetRange(key: PresetKey): DateRange {
  const t = today();
  const now = new Date();
  switch (key) {
    case 'today':
      return { from: t, to: t };
    case 'yesterday': {
      const y = addDays(t, -1);
      return { from: y, to: y };
    }
    case 'last7':
      return { from: addDays(t, -6), to: t };
    case 'last30':
      return { from: addDays(t, -29), to: t };
    case 'thisMonth':
      return { from: toYmd(new Date(now.getFullYear(), now.getMonth(), 1)), to: t };
    case 'lastMonth':
      return {
        from: toYmd(new Date(now.getFullYear(), now.getMonth() - 1, 1)),
        to: toYmd(new Date(now.getFullYear(), now.getMonth(), 0)),
      };
    case 'thisYear':
      return { from: `${now.getFullYear()}-01-01`, to: t };
  }
}
