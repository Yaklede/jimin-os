import { Component, type ErrorInfo, type ReactNode, Suspense } from "react";

import { copy } from "../copy";

type WorkspaceRouteBoundaryProps = {
  children: ReactNode;
  loadingFallback: ReactNode;
  onRetry(): void;
};

type WorkspaceRouteBoundaryState = {
  failed: boolean;
};

export class WorkspaceRouteBoundary extends Component<
  WorkspaceRouteBoundaryProps,
  WorkspaceRouteBoundaryState
> {
  state: WorkspaceRouteBoundaryState = { failed: false };

  static getDerivedStateFromError(): WorkspaceRouteBoundaryState {
    return { failed: true };
  }

  componentDidCatch(_error: Error, _info: ErrorInfo): void {
    // React reports the original render error. Avoid logging route data here.
  }

  render(): ReactNode {
    if (this.state.failed) {
      return <WorkspaceRouteErrorFallback onRetry={this.props.onRetry} />;
    }
    return (
      <Suspense fallback={this.props.loadingFallback}>
        {this.props.children}
      </Suspense>
    );
  }
}

export function WorkspaceRouteErrorFallback({ onRetry }: { onRetry(): void }) {
  return (
    <section className="workspace-route-error" role="alert">
      <strong>{copy.launch.routeLoadFailedTitle}</strong>
      <p>{copy.launch.routeLoadFailedBody}</p>
      <button className="secondary-button" type="button" onClick={onRetry}>
        {copy.launch.retryRouteLoad}
      </button>
    </section>
  );
}
