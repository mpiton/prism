import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceStatusBar } from "./WorkspaceStatusBar";

vi.mock("../../lib/tauri", () => ({
  ptyWrite: vi.fn().mockResolvedValue(undefined),
}));

import { ptyWrite } from "../../lib/tauri";
import type { CiStatus } from "../../lib/types/enums";

const defaultProps = {
  workspaceId: "ws-1",
  branch: "feat/my-feature",
  ahead: 2,
  behind: 1,
  ciStatus: "success" as CiStatus,
  sessionName: "prism-pr-42",
  sessionCount: 3,
  githubUrl: "https://github.com/owner/repo/pull/42",
};

describe("WorkspaceStatusBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should show branch name", () => {
    render(<WorkspaceStatusBar {...defaultProps} />);

    expect(screen.getByTestId("status-branch")).toHaveTextContent(
      "feat/my-feature",
    );
  });

  it("should show ahead/behind counts", () => {
    render(<WorkspaceStatusBar {...defaultProps} />);

    expect(screen.getByTestId("status-ahead")).toHaveTextContent("↑2");
    expect(screen.getByTestId("status-behind")).toHaveTextContent("↓1");
  });

  it("should hide ahead/behind when both are zero", () => {
    render(
      <WorkspaceStatusBar {...defaultProps} ahead={0} behind={0} />,
    );

    expect(screen.queryByTestId("status-ahead")).not.toBeInTheDocument();
    expect(screen.queryByTestId("status-behind")).not.toBeInTheDocument();
  });

  it("should show CI status", () => {
    render(<WorkspaceStatusBar {...defaultProps} />);

    expect(screen.getByTestId("status-ci")).toBeInTheDocument();
  });

  it("should show session name", () => {
    render(<WorkspaceStatusBar {...defaultProps} />);

    expect(screen.getByTestId("status-session")).toHaveTextContent(
      "prism-pr-42",
    );
  });

  it("should show session count", () => {
    render(<WorkspaceStatusBar {...defaultProps} />);

    expect(screen.getByTestId("status-session-count")).toHaveTextContent("3");
  });

  it("should write git push to pty on button click", async () => {
    const user = userEvent.setup();
    render(<WorkspaceStatusBar {...defaultProps} />);

    await user.click(screen.getByTestId("btn-git-push"));

    expect(ptyWrite).toHaveBeenCalledWith({
      workspaceId: "ws-1",
      data: "git push\n",
    });
  });

  it("should write git pull to pty on button click", async () => {
    const user = userEvent.setup();
    render(<WorkspaceStatusBar {...defaultProps} />);

    await user.click(screen.getByTestId("btn-git-pull"));

    expect(ptyWrite).toHaveBeenCalledWith({
      workspaceId: "ws-1",
      data: "git pull\n",
    });
  });

  it("should render open in github link with safe attributes", () => {
    render(<WorkspaceStatusBar {...defaultProps} />);

    const link = screen.getByTestId("btn-open-github");
    expect(link).toHaveAttribute("href", defaultProps.githubUrl);
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("should sanitize non-https github URLs", () => {
    render(
      <WorkspaceStatusBar
        {...defaultProps}
        githubUrl="javascript:alert(1)"
      />,
    );

    const link = screen.getByTestId("btn-open-github");
    expect(link).toHaveAttribute("href", "#");
  });

  it("should show only ahead when behind is zero", () => {
    render(<WorkspaceStatusBar {...defaultProps} ahead={3} behind={0} />);

    expect(screen.getByTestId("status-ahead")).toHaveTextContent("↑3");
    expect(screen.queryByTestId("status-behind")).not.toBeInTheDocument();
  });

  it("should show only behind when ahead is zero", () => {
    render(<WorkspaceStatusBar {...defaultProps} ahead={0} behind={4} />);

    expect(screen.queryByTestId("status-ahead")).not.toBeInTheDocument();
    expect(screen.getByTestId("status-behind")).toHaveTextContent("↓4");
  });

  it("should handle missing session gracefully", () => {
    render(
      <WorkspaceStatusBar
        {...defaultProps}
        sessionName={null}
        sessionCount={5}
      />,
    );

    expect(screen.queryByTestId("status-session")).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("status-session-count"),
    ).not.toBeInTheDocument();
  });

});
