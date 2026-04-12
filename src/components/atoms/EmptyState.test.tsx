import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("should show message", () => {
    render(<EmptyState message="No reviews pending" />);
    expect(screen.getByText("No reviews pending")).toBeInTheDocument();
  });

  it("should center content", () => {
    const { container } = render(<EmptyState message="Empty" />);
    expect(container.firstChild).toHaveClass("text-center");
  });

  it("should have role=status for screen readers", () => {
    render(<EmptyState message="No reviews pending" />);
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  it("should not render a button when no cta prop is provided", () => {
    render(<EmptyState message="Empty" />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("should render a button with cta text when cta is provided", () => {
    render(
      <EmptyState
        message="No repos"
        cta={{ text: "Add a repository", onClick: () => {} }}
      />,
    );
    expect(screen.getByRole("button", { name: "Add a repository" })).toBeInTheDocument();
  });

  it("should call cta.onClick when the button is clicked", async () => {
    const handleClick = vi.fn();
    render(
      <EmptyState
        message="No repos"
        cta={{ text: "Sync now", onClick: handleClick }}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Sync now" }));
    expect(handleClick).toHaveBeenCalledOnce();
  });

  it("should call cta.onClick when activated via keyboard", async () => {
    const handleClick = vi.fn();
    render(
      <EmptyState
        message="No repos"
        cta={{ text: "Sync now", onClick: handleClick }}
      />,
    );

    screen.getByRole("button", { name: "Sync now" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(handleClick).toHaveBeenCalledOnce();
  });

  it("should call cta.onClick when activated with Space", async () => {
    const handleClick = vi.fn();
    render(
      <EmptyState
        message="No repos"
        cta={{ text: "Sync now", onClick: handleClick }}
      />,
    );

    screen.getByRole("button", { name: "Sync now" }).focus();
    await userEvent.keyboard(" ");
    expect(handleClick).toHaveBeenCalledOnce();
  });

  it("should render a button with type='button'", () => {
    render(
      <EmptyState
        message="No repos"
        cta={{ text: "Sync", onClick: () => {} }}
      />,
    );
    expect(screen.getByRole("button")).toHaveAttribute("type", "button");
  });
});
