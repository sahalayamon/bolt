import {
  forwardRef,
  useEffect,
  useId,
  useRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { Laptop, Monitor, Smartphone, Tablet } from 'lucide-react';
import type { DeviceKind } from '../lib/types';

export function cx(...c: Array<string | false | null | undefined>): string {
  return c.filter(Boolean).join(' ');
}

/** Shared easing/durations so every transition feels the same. */
export const EASE = [0.2, 0, 0, 1] as const;
export const T_FAST = { duration: 0.16, ease: EASE };
export const T_BASE = { duration: 0.22, ease: EASE };

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'dangerSolid';
const VARIANTS: Record<Variant, string> = {
  primary: 'bg-accent text-accent-fg font-medium hover:brightness-110',
  secondary: 'bg-elevated text-fg font-medium hover:brightness-110',
  ghost: 'text-muted hover:text-fg hover:bg-elevated',
  danger: 'bg-bad/10 text-bad font-medium hover:bg-bad/15',
  dangerSolid: 'bg-danger text-white font-medium hover:brightness-110',
};

export const Button = forwardRef<
  HTMLButtonElement,
  ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: 'sm' | 'md' }
>(function Button({ variant = 'secondary', size = 'md', className, type = 'button', ...p }, ref) {
  return (
    <button
      ref={ref}
      type={type}
      className={cx(
        'inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-control',
        'cursor-pointer select-none transition duration-150 ease-out active:scale-[0.98]',
        'disabled:pointer-events-none disabled:opacity-40',
        size === 'sm' ? 'min-h-7 px-2.5 text-[13px]' : 'min-h-9 px-3.5 text-sm max-md:min-h-11',
        VARIANTS[variant],
        className,
      )}
      {...p}
    />
  );
});

/** Grouped surface. Rows inside separate with hairlines (`divide-y divide-line`). */
export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cx('rounded-card bg-surface ring-1 ring-line', className)}>{children}</div>;
}

export function Badge({
  tone = 'neutral',
  children,
}: {
  tone?: 'neutral' | 'ok' | 'warn' | 'bad' | 'accent';
  children: ReactNode;
}) {
  const tones = {
    neutral: 'bg-elevated text-muted',
    ok: 'bg-ok/12 text-ok',
    warn: 'bg-warn/14 text-warn',
    bad: 'bg-bad/12 text-bad',
    accent: 'bg-accent-soft text-accent',
  };
  return (
    <span
      className={cx(
        'inline-flex items-center gap-1 whitespace-nowrap rounded-full px-2 py-0.5 text-xs font-medium',
        tones[tone],
      )}
    >
      {children}
    </span>
  );
}

export function DeviceIcon({ kind, className }: { kind: DeviceKind; className?: string }) {
  const Icon = kind === 'phone' ? Smartphone : kind === 'tablet' ? Tablet : kind === 'desktop' ? Monitor : Laptop;
  return <Icon className={className} strokeWidth={1.75} aria-hidden="true" />;
}

/** Soft tile that holds an icon. */
export function IconTile({ children, tone = 'neutral' }: { children: ReactNode; tone?: 'neutral' | 'accent' }) {
  return (
    <span
      className={cx(
        'grid h-9 w-9 shrink-0 place-items-center rounded-[10px]',
        tone === 'accent' ? 'bg-accent-soft text-accent' : 'bg-elevated text-muted',
      )}
    >
      {children}
    </span>
  );
}

export function ProgressBar({
  value,
  label,
  tone = 'accent',
  indeterminate,
}: {
  value: number;
  label: string;
  tone?: 'accent' | 'ok' | 'bad';
  indeterminate?: boolean;
}) {
  const color = tone === 'ok' ? 'bg-ok' : tone === 'bad' ? 'bg-bad' : 'bg-accent';
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : Math.round(value)}
      className="h-1.5 w-full overflow-hidden rounded-full bg-elevated"
    >
      {indeterminate ? (
        <div className={cx('indeterminate h-full w-1/3 rounded-full', color)} />
      ) : (
        <motion.div
          className={cx('h-full rounded-full', color)}
          initial={false}
          animate={{ width: `${value}%` }}
          transition={{ duration: 0.3, ease: EASE }}
        />
      )}
    </div>
  );
}

export function Switch({
  checked,
  onChange,
  label,
  description,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  description?: string;
}) {
  const id = useId();
  return (
    <div className="flex items-center justify-between gap-6 px-4 py-3">
      <div className="min-w-0">
        <label htmlFor={id} className="block cursor-pointer">
          {label}
        </label>
        {description && <p className="t-caption mt-0.5">{description}</p>}
      </div>
      <button
        id={id}
        role="switch"
        type="button"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={cx(
          'relative h-[26px] w-[44px] shrink-0 cursor-pointer rounded-full transition-colors duration-200',
          checked ? 'bg-accent' : 'bg-elevated ring-1 ring-line',
        )}
      >
        <span
          className={cx(
            'absolute top-[2px] left-[2px] h-[22px] w-[22px] rounded-full bg-white shadow-sm transition-transform duration-200 ease-out',
            checked && 'translate-x-[18px]',
          )}
        />
      </button>
    </div>
  );
}

/** Segmented control: used for filters. */
export function Segmented<T extends string>({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: T;
  onChange: (v: T) => void;
  options: Array<[T, string]>;
}) {
  return (
    <div role="group" aria-label={label} className="inline-flex rounded-control bg-elevated p-0.5">
      {options.map(([v, text]) => (
        <button
          key={v}
          type="button"
          aria-pressed={value === v}
          onClick={() => onChange(v)}
          className={cx(
            'min-h-7 cursor-pointer rounded-[8px] px-3 text-[13px] transition duration-150 max-md:min-h-9',
            value === v ? 'bg-surface font-medium text-fg shadow-sm ring-1 ring-line' : 'text-muted hover:text-fg',
          )}
        >
          {text}
        </button>
      ))}
    </div>
  );
}

export function PageHeader({
  title,
  subtitle,
  actions,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
}) {
  return (
    <header className="mb-7 flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
      <div className="min-w-0">
        <h1 className="t-title">{title}</h1>
        {subtitle && <p className="mt-1.5 max-w-prose text-muted">{subtitle}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}

export function SectionTitle({ id, children, aside }: { id?: string; children: ReactNode; aside?: ReactNode }) {
  return (
    <div className="mb-2.5 flex items-center justify-between gap-3 px-1">
      <h2 id={id} className="t-section">
        {children}
      </h2>
      {aside}
    </div>
  );
}

export function EmptyState({
  icon,
  title,
  children,
}: {
  icon: ReactNode;
  title: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-1.5 rounded-card px-6 py-14 text-center ring-1 ring-line">
      <div className="mb-2 text-faint [&>svg]:h-9 [&>svg]:w-9 [&>svg]:stroke-[1.4]" aria-hidden="true">
        {icon}
      </div>
      <h2 className="t-section">{title}</h2>
      {children && <div className="max-w-sm text-[13px] text-muted">{children}</div>}
    </div>
  );
}

const FOCUSABLE =
  'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';

/**
 * Accessible modal: labelled dialog, focus moves in, Tab is trapped, Escape closes
 * (unless `dismissible` is false), focus returns to the opener on close.
 * Centered on desktop, bottom sheet on phones.
 */
export function Modal({
  open,
  title,
  onClose,
  dismissible = true,
  children,
  footer,
  width = 'max-w-md',
  initialFocus,
  hideTitle,
}: {
  open: boolean;
  title: string;
  /** Keep the title for screen readers only (the content already says what this is). */
  hideTitle?: boolean;
  onClose: () => void;
  dismissible?: boolean;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
  /** CSS selector inside the dialog that should receive focus first. */
  initialFocus?: string;
}) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const returnTo = useRef<Element | null>(null);

  useEffect(() => {
    if (!open) return;
    returnTo.current = document.activeElement;
    const el = ref.current;
    const target =
      (initialFocus && el?.querySelector<HTMLElement>(initialFocus)) ||
      el?.querySelector<HTMLElement>(FOCUSABLE) ||
      el;
    target?.focus();
    return () => {
      if (returnTo.current instanceof HTMLElement) returnTo.current.focus();
    };
  }, [open, initialFocus]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape' && dismissible) {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== 'Tab') return;
    const nodes = Array.from(ref.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []);
    if (nodes.length === 0) {
      e.preventDefault();
      return;
    }
    const first = nodes[0]!;
    const last = nodes[nodes.length - 1]!;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-50 flex items-end justify-center bg-black/45 p-0 backdrop-blur-[3px] sm:items-center sm:p-4"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={T_FAST}
          onMouseDown={(e) => {
            if (dismissible && e.target === e.currentTarget) onClose();
          }}
        >
          <motion.div
            ref={ref}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            tabIndex={-1}
            onKeyDown={onKeyDown}
            className={cx(
              'flex max-h-[88vh] w-full flex-col overflow-hidden bg-surface shadow-pop outline-none',
              'rounded-t-[20px] sm:rounded-[18px]',
              width,
            )}
            initial={{ opacity: 0, y: 14, scale: 0.985 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.99 }}
            transition={T_BASE}
          >
            <h2
              id={titleId}
              className={cx('text-[17px] leading-snug font-semibold tracking-tight', hideTitle ? 'sr-only' : 'px-6 pt-6 pb-1')}
            >
              {title}
            </h2>
            <div className={cx('scroll-thin overflow-y-auto px-6', hideTitle ? 'pt-6 pb-3' : 'py-3')}>{children}</div>
            {footer && (
              <div className="flex flex-col-reverse gap-2 px-6 pt-2 pb-6 sm:flex-row sm:justify-end [&>button]:max-sm:w-full">
                {footer}
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
