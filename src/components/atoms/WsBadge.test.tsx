import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WsBadge } from "./WsBadge";

describe("WsBadge", () => {
  it("should show 'open' when no state", () => {
    render(<WsBadge />);
    expect(screen.getByText("open")).toBeInTheDocument();
  });

  it("should show 'resume' when active", () => {
    render(<WsBadge state="active" />);
    expect(screen.getByText("resume")).toBeInTheDocument();
  });

  it("should show 'wake' when suspended", () => {
    render(<WsBadge state="suspended" />);
    expect(screen.getByText("wake")).toBeInTheDocument();
  });

  it("should not render when archived", () => {
    const { container } = render(<WsBadge state="archived" />);
    expect(container.firstChild).toBeNull();
  });

  it("should render as a button", () => {
    render(<WsBadge />);
    expect(screen.getByRole("button")).toBeInTheDocument();
  });

  it("should call onClick when clicked", async () => {
    const handleClick = vi.fn();
    render(<WsBadge onClick={handleClick} />);
    await userEvent.click(screen.getByRole("button"));
    expect(handleClick).toHaveBeenCalledOnce();
  });
});
