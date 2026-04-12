import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Tag } from "./Tag";

describe("Tag", () => {
  // === Default variant ===
  it("should render text in uppercase", () => {
    render(<Tag>draft</Tag>);
    const tag = screen.getByText("draft");
    expect(tag).toBeInTheDocument();
    expect(tag).toHaveClass("uppercase");
  });

  it("should use text-dim by default", () => {
    const { container } = render(<Tag>label</Tag>);
    expect(container.firstChild).toHaveClass("text-dim");
  });

  it("should replace text-dim when custom className provided", () => {
    const { container } = render(<Tag className="text-accent">v2</Tag>);
    expect(container.firstChild).toHaveClass("text-accent");
    expect(container.firstChild).not.toHaveClass("text-dim");
  });

  // === Label variant ===
  it("should render label name when variant is label", () => {
    render(<Tag variant="label" label="bug" />);
    expect(screen.getByText("bug")).toBeInTheDocument();
  });

  it("should color by label name", () => {
    const { container: bug } = render(<Tag variant="label" label="bug" />);
    expect(bug.firstChild).toHaveClass("bg-red/20", "text-red");

    const { container: feat } = render(
      <Tag variant="label" label="feature" />,
    );
    expect(feat.firstChild).toHaveClass("bg-green/20", "text-green");

    const { container: docs } = render(
      <Tag variant="label" label="documentation" />,
    );
    expect(docs.firstChild).toHaveClass("bg-blue/20", "text-blue");

    const { container: fix } = render(<Tag variant="label" label="fix" />);
    expect(fix.firstChild).toHaveClass("bg-orange/20", "text-orange");

    const { container: unknown } = render(
      <Tag variant="label" label="unknown" />,
    );
    expect(unknown.firstChild).toHaveClass("bg-purple/20", "text-purple");
  });

  it("should not match substrings like docker as documentation", () => {
    const { container } = render(<Tag variant="label" label="docker" />);
    expect(container.firstChild).toHaveClass("bg-purple/20", "text-purple");
  });

  it("should render label variant as a pill shape", () => {
    const { container } = render(<Tag variant="label" label="test" />);
    expect(container.firstChild).toHaveClass("rounded-full");
  });
});
