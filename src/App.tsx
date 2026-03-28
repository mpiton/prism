import type { ReactElement } from "react";
import { Sidebar } from "./components/Sidebar";
import { ReviewQueue } from "./components/ReviewQueue";
import { MyPRs } from "./components/MyPRs";
import { Issues } from "./components/Issues";
import { ActivityFeed } from "./components/ActivityFeed";
import { Workspace } from "./components/Workspace";
import { Settings } from "./components/Settings";
import { useDashboardStore } from "./stores/dashboard";
import type { DashboardView } from "./stores/dashboard";

interface MainContentProps {
  readonly view: DashboardView;
}

function MainContent({ view }: MainContentProps): ReactElement {
  switch (view) {
    case "overview":
    case "reviews":
      return <ReviewQueue reviews={[]} onOpen={() => {}} />;
    case "mine":
      return <MyPRs prs={[]} onOpen={() => {}} />;
    case "issues":
      return <Issues issues={[]} onOpen={() => {}} />;
    case "feed":
      return <ActivityFeed />;
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

function App() {
  const currentView = useDashboardStore((s) => s.currentView);
  const isWorkspace = currentView === "workspaces";

  return (
    <div className="flex h-screen bg-bg text-fg">
      <aside className="w-[220px] shrink-0 border-r border-border">
        <Sidebar />
      </aside>

      <main className={isWorkspace ? "flex-1" : "min-w-0 flex-1"}>
        <MainContent view={currentView} />
      </main>
    </div>
  );
}

export default App;
