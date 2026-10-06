import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";

export function MobileDisclosure({
  mobile,
  title,
  children,
  open,
  onOpenChange,
}: {
  mobile: boolean;
  title: string;
  children: ReactNode;
  open?: boolean;
  onOpenChange?(open: boolean): void;
}) {
  if (!children) return null;
  return mobile ? (
    <details
      className="mobile-disclosure"
      open={open}
      onToggle={(event) => onOpenChange?.(event.currentTarget.open)}
    >
      <summary className="focus-visible-control">
        {title}
        <ChevronRight aria-hidden="true" />
      </summary>
      <div>{children}</div>
    </details>
  ) : (
    <>{children}</>
  );
}
