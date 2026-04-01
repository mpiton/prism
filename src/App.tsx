import {
  Component,
  lazy,
  Suspense,
  useCallback,
  useState,
  type ErrorInfo,
  type ReactElement,
  type ReactNode,
} from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { authGetStatus, markAllActivityRead } from "./lib/tauri";
import { useGitHubData } from "./hooks/useGitHubData";
import { AuthSetup } from "./components/AuthSetup/AuthSetup";
import { Sidebar } from "./components/Sidebar";
import { Overview } from "./components/Overview";
import { ReviewQueue } from "./components/ReviewQueue";
import { MyPRs } from "./components/MyPRs";
import { Issues } from "./components/Issues";
import { ActivityFeed } from "./components/ActivityFeed";
import { Toast } from "./components/Toast";
import { CommandPalette } from "./components/CommandPalette";
import { useKeyboard } from "./hooks/useKeyboard";
import { useDashboardStore } from "./stores/dashboard";
import { useWorkspaceEnriched } from "./hooks/useWorkspaceEnriched";
import { useWorkspacesStore } from "./stores/workspaces";
import type { DashboardView } from "./stores/dashboard";

const WorkspaceView = lazy(() =>
  import("./components/Workspace").then((m) => ({ default: m.WorkspaceView })),
);
const Settings = lazy(() =>
  import("./components/Settings").then((m) => ({ default: m.Settings })),
);

function openUrl(url: string): void {
  window.open(url, "_blank", "noopener,noreferrer");
}

function LazyFallback(): ReactElement {
  return (
    <div
      role="status"
      aria-live="polite"
      className="flex h-full items-center justify-center text-fg-muted"
    >
      Loading…
    </div>
  );
}

interface ErrorBoundaryProps {
  readonly children: ReactNode;
}

interface ErrorBoundaryState {
  readonly hasError: boolean;
}

class ChunkErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("Chunk load error:", error, info);
  }

  render(): ReactNode {
    if (this.state.hasError) {
      return (
        <div
          role="alert"
          aria-live="assertive"
          className="flex h-full flex-col items-center justify-center gap-3 text-fg-muted"
        >
          <p>Failed to load this view.</p>
          <button
            type="button"
            className="rounded border border-border px-3 py-1 text-sm hover:bg-bg-hover"
            onClick={() => window.location.reload()}
          >
            Reload
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

interface MainContentProps {
  readonly view: DashboardView;
  readonly onBackToDashboard: () => void;
}

function MainContent({ view, onBackToDashboard }: MainContentProps): ReactElement {
  const { dashboard } = useGitHubData();
  const { statusInfo } = useWorkspaceEnriched(view === "workspaces");
  const queryClient = useQueryClient();

  const markAllRead = useMutation({
    mutationFn: markAllActivityRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["github", "dashboard"] });
      queryClient.invalidateQueries({ queryKey: ["github", "stats"] });
    },
  });

  switch (view) {
    case "overview":
      return <Overview />;
    case "reviews":
      return <ReviewQueue reviews={dashboard?.reviewRequests ?? []} onOpen={openUrl} />;
    case "mine":
      return <MyPRs prs={dashboard?.myPullRequests ?? []} onOpen={openUrl} />;
    case "issues":
      return <Issues issues={dashboard?.assignedIssues ?? []} onOpen={openUrl} />;
    case "feed":
      return (
        <ActivityFeed
          activities={dashboard?.recentActivity ?? []}
          onMarkAllRead={() => markAllRead.mutate()}
        />
      );
    case "workspaces":
      return (
        <WorkspaceView
          workspaces={dashboard?.workspaces ?? []}
          statusInfo={statusInfo}
          onBackToDashboard={onBackToDashboard}
        />
      );
    case "settings":
      return <Settings />;
    default: {
      const _exhaustive: never = view;
      throw new Error(`Unhandled view: ${_exhaustive}`);
    }
  }
}

function AuthGate(): ReactElement {
  return (
    <div className="flex h-screen items-center justify-center bg-bg text-fg">
      <div className="flex w-full max-w-md flex-col gap-6 px-6">
        <div className="text-center">
          <h1 className="text-2xl font-bold text-white">PRism</h1>
          <p className="mt-1 text-sm text-fg-muted">Connect your GitHub account to get started</p>
        </div>
        <AuthSetup />
      </div>
    </div>
  );
}

function App(): ReactElement {
  const authQuery = useQuery({
    queryKey: ["auth", "status"],
    queryFn: authGetStatus,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });

  const currentView = useDashboardStore((s) => s.currentView);
  const isWorkspace = currentView === "workspaces";
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);

  const handleNavigate = useCallback((direction: "up" | "down") => {
    useDashboardStore.getState().navigateList(direction);
  }, []);

  const handleOpen = useCallback(() => {
    const { selectedIndex, navigableItems } = useDashboardStore.getState();
    const item = navigableItems[selectedIndex];
    if (item?.url) openUrl(item.url);
  }, []);

  const handleOpenWorkspace = useCallback(() => {
    const { selectedIndex, navigableItems, setView } =
      useDashboardStore.getState();
    const item = navigableItems[selectedIndex];
    if (item?.workspaceId) {
      useWorkspacesStore.getState().setActiveWorkspace(item.workspaceId);
      setView("workspaces");
    }
  }, []);

  const handleEscape = useCallback(() => {
    useDashboardStore.getState().setView("overview");
  }, []);

  const handleCommandPalette = useCallback(() => {
    setCommandPaletteOpen((prev) => !prev);
  }, []);

  useKeyboard({
    onNavigate: handleNavigate,
    onOpen: handleOpen,
    onOpenWorkspace: handleOpenWorkspace,
    onEscape: handleEscape,
    onCommandPalette: handleCommandPalette,
  });

  if (authQuery.isLoading) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg text-fg-muted">
        Checking authentication…
      </div>
    );
  }

  if (authQuery.isError) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg text-fg-muted">
        Failed to check authentication — please restart the app.
      </div>
    );
  }

  if (!authQuery.data?.connected) {
    return <AuthGate />;
  }

  return (
    <div className="flex h-screen bg-bg text-fg">
      <aside className="w-[220px] shrink-0 border-r border-border">
        <Sidebar />
      </aside>

      <main className={isWorkspace ? "flex-1" : "min-w-0 flex-1"}>
        <ChunkErrorBoundary key={currentView}>
          <Suspense fallback={<LazyFallback />}>
            <MainContent view={currentView} onBackToDashboard={handleEscape} />
          </Suspense>
        </ChunkErrorBoundary>
      </main>

      <Toast />
      <CommandPalette
        open={commandPaletteOpen}
        onOpenChange={setCommandPaletteOpen}
      />
    </div>
  );
}

export default App;
