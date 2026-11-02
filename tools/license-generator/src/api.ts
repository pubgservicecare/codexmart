import { invoke } from '@tauri-apps/api/core';

export interface KeyInfo {
  keyId: number;
  publicKey: string;
  fingerprint: string;
  createdAt: string;
  passphraseSet: boolean;
  lastBackupAt: string | null;
  trustedByApp: boolean;
}

export interface GenStatus {
  keyState: 'none' | 'pending' | 'locked' | 'unlocked';
  key: KeyInfo | null;
  dataDir: string;
  historyCount: number;
  version: string;
}

export interface IssuedRecord {
  serial: string;
  customerNo: number;
  customerName: string;
  customerPhone: string;
  deviceId: string;
  edition: string;
  editionLabel: string;
  issuedOn: string;
  expiresOn: string | null;
  keyFingerprint: string;
  activationKey: string;
  requestCode: string;
  notes: string;
  kind: 'new' | 'reissue' | 'transfer';
  replaces: string | null;
  status: 'issued' | 'superseded' | 'transferred';
  createdAt: string;
}

export interface RequestPreview {
  deviceId: string;
  appVersion: string;
  trialStartedOn: string | null;
  status: string;
  statusLabel: string;
  clockWarning: boolean;
  previous: IssuedRecord[];
}

export interface IssueRequest {
  requestCode: string;
  customerName: string;
  customerPhone: string;
  edition: string;
  issuedOn: string;
  expiresOn?: string | null;
  notes: string;
  kind: 'new' | 'reissue' | 'transfer';
  replaces?: string | null;
}

export interface IssuePreview {
  deviceId: string;
  customerName: string;
  customerPhone: string;
  edition: string;
  editionLabel: string;
  issuedOn: string;
  expiresOn: string | null;
  kind: string;
  replaces: string | null;
  warnings: string[];
}

export interface IssueResult {
  record: IssuedRecord;
  whatsappMessage: string;
}

export const EDITIONS = [
  { code: 'month1', label: '1 month' },
  { code: 'month3', label: '3 months' },
  { code: 'month6', label: '6 months' },
  { code: 'year1', label: '1 year' },
  { code: 'lifetime', label: 'Lifetime (no expiry)' },
  { code: 'trial30', label: '30-day trial extension' },
  { code: 'custom', label: 'Custom end date' },
];

export function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'Something went wrong.';
}

export const gen = {
  status: () => invoke<GenStatus>('gen_status'),
  createKey: (passphrase: string, confirm: string) => invoke<KeyInfo>('gen_create_key', { passphrase, confirm }),
  restoreKey: (path: string, passphrase: string) => invoke<KeyInfo>('gen_restore_key', { path, passphrase }),
  setPassphrase: (passphrase: string, confirm: string) => invoke<void>('gen_set_passphrase', { passphrase, confirm }),
  unlock: (passphrase: string) => invoke<void>('gen_unlock', { passphrase }),
  lock: () => invoke<void>('gen_lock'),
  changePassphrase: (current: string, passphrase: string, confirm: string) =>
    invoke<void>('gen_change_passphrase', { current, passphrase, confirm }),
  backupKey: (path: string) => invoke<KeyInfo>('gen_backup_key', { path }),
  decodeRequest: (code: string) => invoke<RequestPreview>('gen_decode_request', { code }),
  review: (input: IssueRequest) => invoke<IssuePreview>('gen_review', { input }),
  issue: (input: IssueRequest) => invoke<IssueResult>('gen_issue', { input }),
  history: (query: string) => invoke<IssuedRecord[]>('gen_history', { query }),
  exportHistory: (path: string) => invoke<number>('gen_export_history', { path }),
};
