import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SectionHead } from "./SectionHead";

describe("SectionHead", () => {
  it("should render title", () => {
    render(<SectionHead title="Reviews" count={5} />);
    expect(screen.getByText("Reviews")).toBeInTheDocument();
  });

  it("should render count", () => {
    render(<SectionHead title="Reviews" count={5} />);
    expect(screen.getByText("5")).toBeInTheDocument();
  });

  it("should render separator", () => {
    const { container } = render(<SectionHead title="Reviews" count={0} />);
    expect(container.querySelector("hr, [role='separator']")).toBeInTheDocument();
  });
});
