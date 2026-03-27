import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Diff } from "./Diff";

describe("Diff", () => {
  it("should show +/- values", () => {
    render(<Diff additions={42} deletions={17} />);
    expect(screen.getByText("+42")).toBeInTheDocument();
    expect(screen.getByText("-17")).toBeInTheDocument();
  });

  it("should show additions in green", () => {
    render(<Diff additions={10} deletions={0} />);
    expect(screen.getByText("+10")).toHaveClass("text-green");
  });

  it("should show deletions in red", () => {
    render(<Diff additions={0} deletions={5} />);
    expect(screen.getByText("-5")).toHaveClass("text-red");
  });

  it("should use monospace font", () => {
    const { container } = render(<Diff additions={1} deletions={1} />);
    expect(container.firstChild).toHaveClass("font-mono");
  });
});
