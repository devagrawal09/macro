import { describe, expect, it } from "vitest";
import * as ui from "../src";

describe("public API", () => {
  it("exports exactly the six blog components", () => {
    expect(Object.keys(ui).sort()).toEqual(["Badge", "Button", "Card", "Progress", "Stack", "Text"]);
  });
});
