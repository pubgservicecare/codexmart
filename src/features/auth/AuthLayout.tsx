import type { ReactNode } from 'react';

export function AuthLayout({ children, wide }: { children: ReactNode; wide?: boolean }) {
  return (
    <div className="flex h-full overflow-y-auto bg-ink-950">
      <div className="relative hidden w-[44%] flex-col justify-between overflow-hidden p-12 lg:flex">
        <div className="absolute -left-32 -top-32 h-96 w-96 rounded-full bg-brand-600/30 blur-3xl" />
        <div className="absolute -bottom-40 right-0 h-[28rem] w-[28rem] rounded-full bg-violet-700/20 blur-3xl" />
        <div className="relative flex items-center gap-3">
          <img src="/logo.svg" alt="" className="h-11 w-11" />
          <span className="text-lg font-bold tracking-tight text-white">CODE X MART</span>
        </div>
        <div className="relative">
          <h1 className="max-w-md text-4xl font-semibold leading-tight tracking-tight text-white">
            Billing, stock and profit for your mart — in one place.
          </h1>
          <p className="mt-4 max-w-md text-base text-slate-400">
            Works fully offline. Your data stays on this computer, with automatic backups you control.
          </p>
        </div>
        <p className="relative text-xs text-slate-500">Offline retail point of sale for Windows</p>
      </div>
      <div className="flex flex-1 items-center justify-center bg-[var(--canvas)] p-6 lg:rounded-l-[28px]">
        <div className={wide ? 'w-full max-w-xl' : 'w-full max-w-sm'}>{children}</div>
      </div>
    </div>
  );
}
