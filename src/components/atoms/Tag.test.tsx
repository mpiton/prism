import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Tag } from "./Tag";

describe("Tag", () => {
  it("should render text in uppercase", () => {
    render(<Tag>draft</Tag>);
    const tag = screen.getByText("draft");
    expect(tag).toBeInTheDocument();
    expect(tag).toHaveClass("uppercase");
  });

  it("should apply custom className", () => {
    const { container } = render(<Tag className="text-accent">v2</Tag>);
    expect(container.firstChild).toHaveClass("text-accent");
  });
});
