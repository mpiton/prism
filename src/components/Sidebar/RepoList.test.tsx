import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Repo } from "../../lib/types";
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
    await userEvent.click(screen.getByRole("checkbox"));
    expect(handleToggle).toHaveBeenCalledWith("repo-1", false);
  });

  it("should render empty when no repos", () => {
    render(<RepoList repos={[]} onToggleRepo={vi.fn()} />);
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });
});
