import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { PriorityBar } from "./PriorityBar";

describe("PriorityBar", () => {
  it("should have correct height per priority", () => {
    const { container: critical } = render(<PriorityBar priority="critical" />);
    expect(critical.firstChild).toHaveClass("h-full");

    const { container: high } = render(<PriorityBar priority="high" />);
    expect(high.firstChild).toHaveClass("h-3/4");

    const { container: medium } = render(<PriorityBar priority="medium" />);
    expect(medium.firstChild).toHaveClass("h-1/2");

    const { container: low } = render(<PriorityBar priority="low" />);
    expect(low.firstChild).toHaveClass("h-1/4");
  });

  it("should color by priority level", () => {
    const { container: critical } = render(<PriorityBar priority="critical" />);
    expect(critical.firstChild).toHaveClass("bg-red");

    const { container: high } = render(<PriorityBar priority="high" />);
    expect(high.firstChild).toHaveClass("bg-orange");

    const { container: medium } = render(<PriorityBar priority="medium" />);
    expect(medium.firstChild).toHaveClass("bg-blue");

    const { container: low } = render(<PriorityBar priority="low" />);
    expect(low.firstChild).toHaveClass("bg-dim");
  });
});
