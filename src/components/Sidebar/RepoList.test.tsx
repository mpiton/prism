import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { act, render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Repo } from "../../lib/types/github";
import { RepoList } from "./RepoList";

const enabledRepo: Repo = {
  id: "repo-1",
  org: "acme",
  name: "frontend",
  fullName: "acme/frontend",
  url: "https://github.com/acme/frontend",
  defaultBranch: "main",
  isArchived: false,
  enabled: true,
  localPath: "/home/user/frontend",
  lastSyncAt: "2026-03-28T10:00:00Z",
};

const disabledRepo: Repo = {
  id: "repo-2",
  org: "acme",
  name: "backend",
  fullName: "acme/backend",
  url: "https://github.com/acme/backend",
  defaultBranch: "main",
  isArchived: false,
  enabled: false,
  localPath: null,
  lastSyncAt: null,
};

function makeRepo(index: number): Repo {
  return {
    id: `repo-${index}`,
    org: "acme",
    name: `repo-${index}`,
    fullName: `acme/repo-${index}`,
    url: `https://github.com/acme/repo-${index}`,
    defaultBranch: "main",
    isArchived: false,
    enabled: true,
    localPath: null,
    lastSyncAt: null,
  };
}

describe("RepoList", () => {
  it("should render repo names", () => {
    render(
      <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} />,
    );
    expect(screen.getByText("acme/frontend")).toBeInTheDocument();
    expect(screen.getByText("acme/backend")).toBeInTheDocument();
  });

  it("should show checkbox for each repo", () => {
    render(
      <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} />,
    );
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes).toHaveLength(2);
  });

  it("should show checked state for enabled repos", () => {
    render(
      <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} />,
    );
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes[0]).toBeChecked();
    expect(checkboxes[1]).not.toBeChecked();
  });

  it("should call onToggleRepo when checkbox clicked", async () => {
    const handleToggle = vi.fn();
    render(
      <RepoList repos={[enabledRepo]} onToggleRepo={handleToggle} />,
    );
    const checkbox = screen.getAllByRole("checkbox")[0]!;
    await userEvent.click(checkbox);
    expect(handleToggle).toHaveBeenCalledWith("repo-1", false);
  });

  it("should render empty when no repos", () => {
    render(<RepoList repos={[]} onToggleRepo={vi.fn()} />);
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });

  describe("search filtering", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("should filter repos by search input", () => {
      render(
        <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} />,
      );

      const searchInput = screen.getByPlaceholderText("Filter repos...");
      act(() => {
        fireEvent.change(searchInput, { target: { value: "frontend" } });
        vi.advanceTimersByTime(200);
      });

      expect(screen.getByText("acme/frontend")).toBeInTheDocument();
      expect(screen.queryByText("acme/backend")).not.toBeInTheDocument();
    });

    it("should show 'No repos match' when search has no results", () => {
      render(
        <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} />,
      );

      const searchInput = screen.getByPlaceholderText("Filter repos...");
      act(() => {
        fireEvent.change(searchInput, { target: { value: "zzznomatch" } });
        vi.advanceTimersByTime(200);
      });

      expect(screen.getByText("No repos match")).toBeInTheDocument();
    });
  });

  describe("truncation", () => {
    it("should show only first 6 repos when more exist", () => {
      const repos = Array.from({ length: 10 }, (_, i) => makeRepo(i + 1));
      render(<RepoList repos={repos} onToggleRepo={vi.fn()} />);

      const checkboxes = screen.getAllByRole("checkbox");
      expect(checkboxes).toHaveLength(6);
      expect(screen.getByText(/Show \d+ more/)).toBeInTheDocument();
    });

    it("should show all repos when 'Show all' is clicked", async () => {
      const repos = Array.from({ length: 10 }, (_, i) => makeRepo(i + 1));
      render(<RepoList repos={repos} onToggleRepo={vi.fn()} />);

      await userEvent.click(screen.getByText(/Show \d+ more/));

      const checkboxes = screen.getAllByRole("checkbox");
      expect(checkboxes).toHaveLength(10);
      expect(screen.queryByText(/Show \d+ more/)).not.toBeInTheDocument();
    });
  });

  describe("batch controls", () => {
    it("should call onSelectAll when select all is clicked", async () => {
      const onSelectAll = vi.fn();
      render(
        <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} onSelectAll={onSelectAll} />,
      );

      await userEvent.click(screen.getByText("Select all"));
      expect(onSelectAll).toHaveBeenCalledOnce();
    });

    it("should call onDeselectAll when deselect all is clicked", async () => {
      const onDeselectAll = vi.fn();
      render(
        <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} onDeselectAll={onDeselectAll} />,
      );

      await userEvent.click(screen.getByText("Deselect all"));
      expect(onDeselectAll).toHaveBeenCalledOnce();
    });

    it("should not show batch controls when callbacks not provided", () => {
      render(
        <RepoList repos={[enabledRepo, disabledRepo]} onToggleRepo={vi.fn()} />,
      );

      expect(screen.queryByText("Select all")).not.toBeInTheDocument();
      expect(screen.queryByText("Deselect all")).not.toBeInTheDocument();
    });
  });
});
