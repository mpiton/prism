import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { LabelTag } from "./LabelTag";

describe("LabelTag", () => {
  it("should render label name", () => {
    render(<LabelTag name="bug" />);
    expect(screen.getByText("bug")).toBeInTheDocument();
  });

  it("should color by label name", () => {
    const { container: bug } = render(<LabelTag name="bug" />);
    expect(bug.firstChild).toHaveClass("bg-red/20", "text-red");

    const { container: feat } = render(<LabelTag name="feature" />);
    expect(feat.firstChild).toHaveClass("bg-green/20", "text-green");

    const { container: docs } = render(<LabelTag name="documentation" />);
    expect(docs.firstChild).toHaveClass("bg-blue/20", "text-blue");
  });

  it("should render as a pill shape", () => {
    const { container } = render(<LabelTag name="test" />);
    expect(container.firstChild).toHaveClass("rounded-full");
  });
});
