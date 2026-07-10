import { RefreshCw } from "lucide-react";

import { copy } from "../copy";

interface AppHeaderProps {
  isRefreshing: boolean;
  onRefresh: () => void;
}

export function AppHeader({ isRefreshing, onRefresh }: AppHeaderProps) {
  return (
    <header className="app-header">
      <div className="app-header__inner">
        <div className="brand" aria-label={copy.productName}>
          <span className="brand__mark" aria-hidden="true">
            J
          </span>
          <span className="brand__name">{copy.productName}</span>
        </div>

        <button
          className="primary-button focus-visible-control"
          type="button"
          aria-label={copy.actions.checkAgainLabel}
          disabled={isRefreshing}
          onClick={onRefresh}
        >
          <RefreshCw
            className={isRefreshing ? "spin" : undefined}
            aria-hidden="true"
          />
          <span>
            {isRefreshing ? copy.actions.checking : copy.actions.checkAgain}
          </span>
        </button>
      </div>
    </header>
  );
}
