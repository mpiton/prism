import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
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
});
