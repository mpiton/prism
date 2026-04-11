import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FOCUS_RING } from "../../lib/a11y";
import { NavItem } from "./NavItem";

describe("NavItem", () => {
  it("should render label", () => {
    render(<NavItem label="Overview" view="overview" isActive={false} onClick={vi.fn()} />);
    expect(screen.getByText("Overview")).toBeInTheDocument();
  });

  it("should render count when provided", () => {
    render(
      <NavItem label="To Review" view="reviews" count={5} isActive={false} onClick={vi.fn()} />,
    );
    expect(screen.getByText("5")).toBeInTheDocument();
  });

  it("should not render count when zero", () => {
    render(<NavItem label="Issues" view="issues" count={0} isActive={false} onClick={vi.fn()} />);
    expect(screen.queryByText("0")).not.toBeInTheDocument();
  });

  it("should highlight when active", () => {
    render(<NavItem label="Overview" view="overview" isActive={true} onClick={vi.fn()} />);
    const button = screen.getByRole("button", { name: /overview/i });
    expect(button).toHaveAttribute("aria-current", "page");
  });

  it("should not highlight when inactive", () => {
    render(<NavItem label="Overview" view="overview" isActive={false} onClick={vi.fn()} />);
    const button = screen.getByRole("button", { name: /overview/i });
    expect(button).not.toHaveAttribute("aria-current");
  });

  it("should call onClick with view when clicked", async () => {
    const handleClick = vi.fn();
    render(<NavItem label="Reviews" view="reviews" isActive={false} onClick={handleClick} />);
    await userEvent.click(screen.getByRole("button", { name: /reviews/i }));
    expect(handleClick).toHaveBeenCalledWith("reviews");
  });

  it("should have aria-label including count when present", () => {
    render(
      <NavItem label="To Review" view="reviews" count={5} isActive={false} onClick={vi.fn()} />,
    );
    const button = screen.getByRole("button");
    expect(button).toHaveAttribute("aria-label", "To Review (5)");
  });

  it("should not have aria-label when count is absent", () => {
    render(<NavItem label="Overview" view="overview" isActive={false} onClick={vi.fn()} />);
    const button = screen.getByRole("button");
    expect(button).not.toHaveAttribute("aria-label");
  });

  it("should not have aria-label when count is zero", () => {
    render(<NavItem label="Issues" view="issues" count={0} isActive={false} onClick={vi.fn()} />);
    const button = screen.getByRole("button");
    expect(button).not.toHaveAttribute("aria-label");
  });

  it("should apply the focus-visible ring for keyboard accessibility (WCAG 2.4.7)", () => {
    render(<NavItem label="Overview" view="overview" isActive={false} onClick={vi.fn()} />);
    const button = screen.getByRole("button", { name: /overview/i });
    for (const token of FOCUS_RING.trim().split(/\s+/)) {
      expect(button).toHaveClass(token);
    }
  });

  it("should pass through keyboard and focus props", async () => {
    const handleFocus = vi.fn();
    const handleKeyDown = vi.fn();
    render(
      <NavItem
        label="Overview"
        view="overview"
        isActive={false}
        onClick={vi.fn()}
        tabIndex={-1}
        onFocus={handleFocus}
        onKeyDown={handleKeyDown}
      />,
    );

    const button = screen.getByRole("button", { name: /overview/i });
    button.focus();
    await userEvent.keyboard("{ArrowDown}");

    expect(button).toHaveAttribute("tabindex", "-1");
    expect(handleFocus).toHaveBeenCalledTimes(1);
    expect(handleKeyDown).toHaveBeenCalled();
  });
});
