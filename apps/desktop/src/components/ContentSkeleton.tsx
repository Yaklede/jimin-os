import { type ReactNode, useEffect, useRef, useState } from "react";

const DEFAULT_DELAY_MS = 300;
const DEFAULT_MINIMUM_MS = 300;

type SkeletonGroupProps = {
  children: ReactNode;
  className?: string;
  label: string;
  visible: boolean;
};

export function useDelayedSkeleton(
  loading: boolean,
  delayMs = DEFAULT_DELAY_MS,
  minimumMs = DEFAULT_MINIMUM_MS,
): boolean {
  const [visible, setVisible] = useState(false);
  const visibleSince = useRef<number | undefined>(undefined);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;

    if (loading && !visible) {
      timer = setTimeout(() => {
        visibleSince.current = Date.now();
        setVisible(true);
      }, delayMs);
    } else if (!loading && visible) {
      const elapsed = Date.now() - (visibleSince.current ?? Date.now());
      timer = setTimeout(
        () => {
          visibleSince.current = undefined;
          setVisible(false);
        },
        Math.max(0, minimumMs - elapsed),
      );
    }

    return () => {
      if (timer) clearTimeout(timer);
    };
  }, [delayMs, loading, minimumMs, visible]);

  return visible;
}

export function SkeletonGroup({
  children,
  className,
  label,
  visible,
}: SkeletonGroupProps) {
  return (
    <div
      className={["content-skeleton", className].filter(Boolean).join(" ")}
      data-visible={visible}
      role="status"
    >
      <span className="sr-only">{visible ? label : ""}</span>
      <div className="content-skeleton__shape" aria-hidden="true">
        {children}
      </div>
    </div>
  );
}

export function SkeletonBlock({ className }: { className?: string }) {
  return (
    <span
      className={["skeleton", className].filter(Boolean).join(" ")}
      aria-hidden="true"
    />
  );
}
