import { save } from '@tauri-apps/plugin-dialog';
import { api } from './api';

export interface Column<T> {
  header: string;
  value: (row: T) => string | number | null | undefined;
}

function cell(v: string | number | null | undefined): string {
  if (v === null || v === undefined) return '';
  const s = String(v);
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

export function toCsv<T>(rows: T[], columns: Column<T>[]): string {
  const lines = [columns.map((c) => cell(c.header)).join(',')];
  for (const r of rows) lines.push(columns.map((c) => cell(c.value(r))).join(','));
  return lines.join('\r\n');
}

/** Paisa as a plain decimal for spreadsheets. */
export function csvMoney(minor: number | null | undefined): string {
  if (minor === null || minor === undefined) return '';
  return (minor / 100).toFixed(2);
}

export function csvQty(milli: number): string {
  return String(milli / 1000);
}

/** Asks where to save and writes the CSV. Returns false if cancelled. */
export async function exportCsv<T>(defaultName: string, rows: T[], columns: Column<T>[]): Promise<boolean> {
  const path = await save({ defaultPath: defaultName, filters: [{ name: 'CSV (Excel)', extensions: ['csv'] }] });
  if (!path) return false;
  await api.app.exportText(path, toCsv(rows, columns));
  return true;
}
