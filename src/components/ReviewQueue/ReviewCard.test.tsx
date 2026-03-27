import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PullRequestWithReview } from "../../lib/types";
import { ReviewCard } from "./ReviewCard";

const mockData: PullRequestWithReview = {
  pullRequest: {
    id: "pr-1",
    number: 42,
    title: "Fix login bug",
    author: "alice",
    state: "open",
    ciStatus: "success",
    priority: "high",
    repoId: "repo-1",
    url: "https://github.com/org/repo/pull/42",
    labels: ["bug"],
    additions: 10,
    deletions: 5,
    changedFiles: 3,
    commentsCount: 2,
    createdAt: "2026-03-26T10:00:00Z",
    updatedAt: "2026-03-26T12:00:00Z",
  },
  reviewSummary: {
    totalReviews: 3,
    approved: 1,
    changesRequested: 0,
    pending: 2,
    reviewers: ["bob", "carol", "dave"],
  },
  workspace: {
    id: "ws-1",
    state: "active",
    lastNoteContent: null,
  },
};

describe("ReviewCard", () => {
  it("should render PR title and number", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("Fix login bug")).toBeInTheDocument();
    expect(screen.getByText("#42")).toBeInTheDocument();
  });

  it("should render author name", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("alice")).toBeInTheDocument();
  });

  it("should show diff stats", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("+10")).toBeInTheDocument();
    expect(screen.getByText("-5")).toBeInTheDocument();
  });

  it("should show CI status", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("PASS")).toBeInTheDocument();
  });

  it("should show workspace badge", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("resume")).toBeInTheDocument();
  });

  it("should call onOpen on click", async () => {
    const handleOpen = vi.fn();
    render(<ReviewCard data={mockData} onOpen={handleOpen} />);
    await userEvent.click(screen.getByRole("link"));
    expect(handleOpen).toHaveBeenCalledWith(
      "https://github.com/org/repo/pull/42",
    );
  });
});
