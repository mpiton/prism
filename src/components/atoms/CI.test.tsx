import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { CI } from "./CI";

describe("CI", () => {
  it("should render PASS in green", () => {
    render(<CI status="success" />);
    const badge = screen.getByText("PASS");
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveClass("text-green");
  });

  it("should render FAIL in red", () => {
    render(<CI status="failure" />);
    const badge = screen.getByText("FAIL");
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveClass("text-red");
  });

  it("should render RUN in orange", () => {
    render(<CI status="running" />);
    const badge = screen.getByText("RUN");
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveClass("text-orange");
  });

  it("should render PEND in dim for pending", () => {
    render(<CI status="pending" />);
    const badge = screen.getByText("PEND");
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveClass("text-dim");
  });

  it("should render CANCEL in dim for cancelled", () => {
    render(<CI status="cancelled" />);
    const badge = screen.getByText("CANCEL");
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveClass("text-dim");
  });
});
