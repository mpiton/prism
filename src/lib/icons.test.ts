import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { createElement } from "react";
import * as icons from "./icons";

describe("icons barrel", () => {
  it("should render CircleCheck as a component", () => {
    const { container } = render(createElement(icons.CircleCheck));
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("should render CircleX as a component", () => {
    const { container } = render(createElement(icons.CircleX));
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("should render Eye as a component", () => {
    const { container } = render(createElement(icons.Eye));
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("should expose exactly the whitelisted icons (no accidental wildcard)", () => {
    const exportedKeys = Object.keys(icons).sort();
    expect(exportedKeys).toEqual(
      ["BookOpen", "CircleCheck", "CircleX", "Download", "Eye", "Focus", "FolderOpen", "LayoutDashboard", "RefreshCw", "Settings"].sort(),
    );
  });
});
