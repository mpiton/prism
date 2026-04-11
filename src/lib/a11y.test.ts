import { describe, expect, it } from "vitest";
import { FOCUS_RING } from "./a11y";

describe("FOCUS_RING", () => {
  it("should remove default outline so browser focus ring does not conflict with the Tailwind ring", () => {
    expect(FOCUS_RING).toContain("outline-none");
  });

  it("should include focus-visible:ring-2 to install a 2px ring on keyboard focus only", () => {
    expect(FOCUS_RING).toContain("focus-visible:ring-2");
  });

  it("should use the accent color for the ring to match the PRism design system", () => {
    expect(FOCUS_RING).toContain("focus-visible:ring-accent");
  });

  it("should include a 2px ring offset to separate the ring from the element border", () => {
    expect(FOCUS_RING).toContain("focus-visible:ring-offset-2");
  });

  it("should offset with a transparent color so the element's own background shows through on any surface", () => {
    expect(FOCUS_RING).toContain("focus-visible:ring-offset-transparent");
  });

  it("should not rely on the plain :focus selector which would trigger on mouse clicks", () => {
    expect(FOCUS_RING).not.toMatch(/\bfocus:ring/);
  });
});
