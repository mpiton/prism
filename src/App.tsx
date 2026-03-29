import { useCallback, useState, type ReactElement } from "react";
import { Sidebar } from "./components/Sidebar";
import { Overview } from "./components/Overview";
import { ReviewQueue } from "./components/ReviewQueue";
import { MyPRs } from "./components/MyPRs";
import { Issues } from "./components/Issues";
import { ActivityFeed } from "./components/ActivityFeed";
import { Workspace } from "./components/Workspace";
import { Settings } from "./components/Settings";
import { Toast } from "./components/Toast";
import { CommandPalette } from "./components/CommandPalette";
import { useKeyboard } from "./hooks/useKeyboard";
import { useDashboardStore } from "./stores/dashboard";
import { useWorkspacesStore } from "./stores/workspaces";
import type { DashboardView } from "./stores/dashboard";

function openUrl(url: string): void {
  window.open(url, "_blank", "noopener,noreferrer");
}

interface MainContentProps {
  readonly view: DashboardView;
}

function MainContent({ view }: MainContentProps): ReactElement {
  switch (view) {
    case "overview":
      return <Overview />;
    case "reviews":
      return <ReviewQueue reviews={[]} onOpen={openUrl} />;
    case "mine":
      return <MyPRs prs={[]} onOpen={openUrl} />;
    case "issues":
      return <Issues issues={[]} onOpen={openUrl} />;
    case "feed":
      return <ActivityFeed activities={[]} onMarkAllRead={() => {}} />;
    case "workspaces":
      return <Workspace />;
    case "settings":
      return <Settings />;
    default: {
      const _exhaustive: never = view;
      throw new Error(`Unhandled view: ${_exhaustive}`);
    }
  }
}

function App(): ReactElement {
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

  const handleSwitchWorkspace = useCallback((_index: number) => {
    // Workspace switching by index requires workspace list data (T-067+)
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
    onSwitchWorkspace: handleSwitchWorkspace,
    onEscape: handleEscape,
    onCommandPalette: handleCommandPalette,
  });

  return (
    <div className="flex h-screen bg-bg text-fg">
      <aside className="w-[220px] shrink-0 border-r border-border">
        <Sidebar />
      </aside>

      <main className={isWorkspace ? "flex-1" : "min-w-0 flex-1"}>
        <MainContent view={currentView} />
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
