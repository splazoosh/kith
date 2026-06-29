// TreeCanvas.test.ts — jsdom render of a hand-built model. The d3-zoom action is
// mocked so no transition/`select` runs
// under jsdom; the test asserts the canvas renders the model VERBATIM — one card
// <g> per person at its own (x, y) sized to its own (width, height) (never a
// hardcoded 220×72), the name + lifespan as real <text>, a <title> per card,
// the viewBox derived from bounds, and one link stroke through the anchors
// rounded over the SAME points. A person card click re-roots
// on its entity id; a union is inert.

import { render } from "@testing-library/svelte";
import { expect, test, vi } from "vitest";

// Mock the d3 seam: a no-op action keeps d3/transition out of jsdom.
vi.mock("../lib/actions/zoom", () => ({
  zoomable: vi.fn(),
  prefersReducedMotion: vi.fn(() => false),
}));

import type { LayoutModel } from "../lib/types";
import { roundedPathFromAnchors, viewBoxFor } from "../lib/viewport";
import TreeCanvas from "./TreeCanvas.svelte";

const noop = (): void => {};

// Distinctive box sizes (200×80, NOT 220×72) prove the card reads its own model
// box rather than any hardcoded constant.
const anchors = [{ x: 110, y: 100 }, { x: 110, y: 200 }];
const model: LayoutModel = {
  mode: "Descendants",
  bounds: { x: 10, y: 20, width: 200, height: 260 },
  nodes: [
    {
      id: 0,
      kind: "Person",
      entity: { Person: 1 },
      x: 10,
      y: 20,
      width: 200,
      height: 80,
      focal: true,
      content: {
        display_name: "Ada Lovelace",
        lifespan: "1815–1852",
        sex: "Female",
        living: false,
        portrait: null,
      },
    },
    {
      id: 1,
      kind: "Person",
      entity: { Person: 2 },
      x: 10,
      y: 200,
      width: 200,
      height: 80,
      focal: false,
      content: {
        display_name: "Allegra Byron",
        lifespan: null,
        sex: "Female",
        living: false,
        portrait: null,
      },
    },
  ],
  links: [{ from: 0, to: 1, kind: "Descent", anchors }],
};

test("renders one card per person at its own model box", () => {
  const { container } = render(TreeCanvas, { props: { model, onreroot: noop } });

  const cards = container.querySelectorAll("g.card");
  expect(cards).toHaveLength(2);
  expect(cards[0].getAttribute("transform")).toBe("translate(10 20)");
  expect(cards[1].getAttribute("transform")).toBe("translate(10 200)");

  // The card box comes from the model (200×80), never a hardcoded 220×72.
  const bg = cards[0].querySelector("rect.bg");
  expect(bg?.getAttribute("width")).toBe("200");
  expect(bg?.getAttribute("height")).toBe("80");

  // The focal node is emphasized.
  expect(cards[0]).toHaveClass("focal");
  expect(cards[1]).not.toHaveClass("focal");
});

test("renders name + lifespan as real <text> and a <title> per card", () => {
  const { container, getByText } = render(TreeCanvas, {
    props: { model, onreroot: noop },
  });

  expect(getByText("Ada Lovelace").tagName.toLowerCase()).toBe("text");
  expect(getByText("1815–1852").tagName.toLowerCase()).toBe("text");

  const titles = [...container.querySelectorAll("g.card title")].map(
    (t) => t.textContent,
  );
  expect(titles).toEqual(["Ada Lovelace (1815–1852)", "Allegra Byron"]);
});

test("derives the viewBox from bounds and strokes one link through its anchors", () => {
  const { container } = render(TreeCanvas, { props: { model, onreroot: noop } });

  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("viewBox")).toBe(viewBoxFor(model.bounds, 48));

  const links = container.querySelectorAll("path.link");
  expect(links).toHaveLength(1);
  // The stroke is rounded over the SAME anchors — for this 2-anchor link
  // that is exactly the straight path.
  expect(links[0].getAttribute("d")).toBe(roundedPathFromAnchors(anchors, 10));
});

test("the <svg> is role=group (not img) so its focusable nodes reach AT", () => {
  const { container } = render(TreeCanvas, { props: { model, onreroot: noop } });
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("role")).toBe("group");
});

test("clicking a person card calls onreroot with its entity person id", () => {
  const onreroot = vi.fn();
  const { container } = render(TreeCanvas, { props: { model, onreroot } });

  (container.querySelector("g.card") as SVGGElement).dispatchEvent(
    new MouseEvent("click", { bubbles: true }),
  );
  expect(onreroot).toHaveBeenCalledWith(1); // model node 0's entity is { Person: 1 }
});

test("swapping the model (a mode switch) re-frames the viewBox and re-positions nodes", async () => {
  // Descendants-shaped model: focus at top, compact bounds.
  const descendants: LayoutModel = model;
  // Ancestors-shaped model: SAME node ids reused for DIFFERENT people at
  // DIFFERENT coordinates + a wider bounds — exactly what a mode switch returns.
  const ancestors: LayoutModel = {
    mode: "Ancestors",
    bounds: { x: -110, y: 0, width: 1098, height: 720 },
    nodes: [
      {
        id: 0,
        kind: "Person",
        entity: { Person: 1 },
        x: 390,
        y: 648,
        width: 220,
        height: 72,
        focal: true,
        content: {
          display_name: "Olav Lund",
          lifespan: null,
          sex: "Male",
          living: false,
          portrait: null,
        },
      },
      {
        id: 1,
        kind: "Person",
        entity: { Person: 7 },
        x: -110,
        y: 0,
        width: 220,
        height: 72,
        focal: false,
        content: {
          display_name: "Gustav Lund",
          lifespan: null,
          sex: "Male",
          living: false,
          portrait: null,
        },
      },
    ],
    links: [],
  };

  const { container, rerender } = render(TreeCanvas, {
    props: { model: descendants, onreroot: noop },
  });
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("viewBox")).toBe(viewBoxFor(descendants.bounds, 48));
  const cardsBefore = container.querySelectorAll("g.card");
  expect(cardsBefore[0].getAttribute("transform")).toBe("translate(10 20)");

  await rerender({ model: ancestors, onreroot: noop });

  // The viewBox must follow the new bounds (else the new chart is mis-framed)…
  expect(svg?.getAttribute("viewBox")).toBe(viewBoxFor(ancestors.bounds, 48));
  // …and every node must move to its new coordinates (else they pile up).
  const cardsAfter = container.querySelectorAll("g.card");
  expect(cardsAfter[0].getAttribute("transform")).toBe("translate(390 648)");
  expect(cardsAfter[1].getAttribute("transform")).toBe("translate(-110 0)");
});

test("a union node is inert — not a button, and a click does not re-root", () => {
  const onreroot = vi.fn();
  const unionModel: LayoutModel = {
    mode: "Descendants",
    bounds: { x: 0, y: 0, width: 40, height: 40 },
    nodes: [
      {
        id: 0,
        kind: "Union",
        entity: { Union: 5 },
        x: 0,
        y: 0,
        width: 20,
        height: 20,
        focal: false,
        content: null,
      },
    ],
    links: [],
  };
  const { container } = render(TreeCanvas, { props: { model: unionModel, onreroot } });

  expect(container.querySelector("g.card")).toBeNull();
  const union = container.querySelector("circle.union");
  expect(union).not.toBeNull();
  expect(union?.getAttribute("role")).not.toBe("button");

  union?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  expect(onreroot).not.toHaveBeenCalled();
});

// — portraits: the canvas draws a circular <image> when the node's
//   portrait id resolves in `portraitUrls`, and slides the name right of it. —
function portraitModel(portrait: number | null): LayoutModel {
  return {
    mode: "Descendants",
    bounds: { x: 0, y: 0, width: 220, height: 72 },
    nodes: [
      {
        id: 0,
        kind: "Person",
        entity: { Person: 1 },
        x: 0,
        y: 0,
        width: 220,
        height: 72,
        focal: true,
        content: {
          display_name: "Ada Lovelace",
          lifespan: null,
          sex: "Female",
          living: false,
          portrait,
        },
      },
    ],
    links: [],
  };
}

test("draws a circular portrait <image> when the url resolves and shifts the name", () => {
  const { container } = render(TreeCanvas, {
    props: {
      model: portraitModel(1),
      portraitUrls: { 1: "asset://face.png" },
      onreroot: noop,
    },
  });
  const img = container.querySelector("g.card image");
  expect(img?.getAttribute("href")).toBe("asset://face.png");
  // The name slides right of the avatar (PORTRAIT_INSET + PORTRAIT_D + INSET = 72).
  expect(container.querySelector("text.name")?.getAttribute("x")).toBe("72");
});

test("omits the portrait and keeps the name inset when the id does not resolve", () => {
  const { container } = render(TreeCanvas, {
    props: { model: portraitModel(1), portraitUrls: {}, onreroot: noop },
  });
  expect(container.querySelector("g.card image")).toBeNull();
  expect(container.querySelector("text.name")?.getAttribute("x")).toBe("14");
});
