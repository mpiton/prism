function App() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 bg-bg p-8">
      <h1 className="text-4xl font-bold text-accent">PRism</h1>
      <p className="text-lg text-dim">
        GitHub Review Dashboard &amp; PR Workspaces
      </p>

      <div className="mt-4 grid grid-cols-3 gap-3">
        <div className="rounded-lg border border-border bg-surface px-4 py-3">
          <span className="text-sm text-muted">Status</span>
          <p className="font-mono text-sm text-green">Passing</p>
        </div>
        <div className="rounded-lg border border-border bg-surface px-4 py-3">
          <span className="text-sm text-muted">Reviews</span>
          <p className="font-mono text-sm text-orange">3 pending</p>
        </div>
        <div className="rounded-lg border border-border bg-surface px-4 py-3">
          <span className="text-sm text-muted">PRs</span>
          <p className="font-mono text-sm text-blue">5 open</p>
        </div>
      </div>

      <p className="mt-6 text-xs text-muted">
        Tailwind CSS 4 + Design Tokens PRism
      </p>
    </main>
  );
}

export default App;
