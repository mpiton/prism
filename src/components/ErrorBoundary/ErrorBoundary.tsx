import { Component, Fragment, type ErrorInfo, type ReactNode } from "react";
import { FOCUS_RING } from "../../lib/a11y";

interface ErrorBoundaryProps {
  readonly children: ReactNode;
}

interface ErrorBoundaryState {
  readonly error: Error | null;
  // Incrementing key forces React to unmount and remount the entire child tree,
  // ensuring stale component state is discarded on retry.
  readonly resetKey: number;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { error: null, resetKey: 0 };
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("ErrorBoundary caught:", error, info);
  }

  render(): ReactNode {
    if (this.state.error) {
      return (
        <div
          role="alert"
          className="flex h-screen flex-col items-center justify-center gap-4 bg-bg text-fg"
        >
          <p className="text-lg font-medium">Something went wrong</p>
          <button
            type="button"
            className={`${FOCUS_RING} rounded border border-border px-4 py-2 text-sm hover:bg-bg-hover`}
            onClick={() =>
              this.setState((prev) => ({
                error: null,
                resetKey: prev.resetKey + 1,
              }))
            }
          >
            Retry
          </button>
        </div>
      );
    }
    return <Fragment key={this.state.resetKey}>{this.props.children}</Fragment>;
  }
}
