# @sorogate/site

A static page, built with [Astro](https://astro.build), that works an access policy out in the browser and sets the
result beside the answer the Soroban contract's tests require for the same input. **Early, Testnet only, not audited**
(see the [root README](../../README.md)).

It has **no server, no wallet, no keys and makes no network calls**. It uses only the pure part of the SDK
(`@sorogate/sdk/model`), so the page does not ship the Stellar SDK.

## Where the examples come from

The examples are the cases in [`spec/vectors/evaluate.json`](../../spec/vectors/evaluate.json): the same cases the
contract (`contracts/access-policy/tests/vectors.rs`) and the SDK (`packages/sdk/test/vectors.test.ts`) are run against.
Nothing is written for the page. Each example carries the decision the contract's tests require it to give (**Unit-tested**,
see [`docs/EVIDENCE.md`](../../docs/EVIDENCE.md); the page does not call the contract), and the page
shows it next to the model's only while the input is exactly the example; change a value and the page says that no
contract answer is fixed for that input.

`test/examples.test.ts` checks that every vector appears as an example and that the model, run through the page's own
code, gives the required answer for each. If a vector is added or changed, the page follows it.

## Layout

| Path | What it is |
| --- | --- |
| `src/pages/index.astro` | The page: hero, the playground's mount point, the proof strip, how a contract uses a policy, the deployment, and what the page cannot tell you |
| `src/playground/draft.ts` | What is typed, how it is read, and how it becomes an outcome. No page code, fully tested |
| `src/playground/examples.ts` | The vectors as examples |
| `src/playground/share.ts` | A policy in a link: encodes a draft into the address and decodes one back, distrusting everything it reads |
| `src/playground/view.ts` | The form and the result, drawn with the DOM; typed text is only ever put on the page as text |
| `src/styles/site.css` | The design: colour tokens for light and dark, type, layout, components and motion |
| `public/` | The favicon, the link-preview image, and the licence of the font |
| `test/` | Logic tests, the examples against the model, a jsdom test that drives the real page code, and the contrast test |

## Design

The look is deliberate, and it is kept simple enough to maintain: one stylesheet, no framework, no images beyond the logo.

- **Colour** comes from the logo's blue. Every colour is a token at the top of `site.css`, in a light and a dark set, and
  `test/contrast.test.ts` reads those tokens and fails if any text, mark or border falls below WCAG AA (4.5:1 for text,
  3:1 for marks and borders), in either theme. Change a colour there, not in a rule.
- **Type** is [Inter](https://rsms.me/inter/), self-hosted from the `@fontsource-variable/inter` package, so the page asks no
  other site for anything. Inter is under the SIL Open Font License 1.1; its text is `public/inter-OFL.txt`.
- **The order of the page follows a first visit.** A developer should be able to say, in turn: I understand what it does (the
  first screen says what a policy is for before it says how it works), I can play with a policy (the playground), I can see
  exactly how my contract would call it (the Rust and TypeScript below it, with the real Testnet address read from the
  deployment record), and here is the repository and the contract (the deployment, with copy buttons). The proof strip comes
  after those, because it is why to believe the answer, not how to use it.
- **The first screen** is a dark hero with an illustration of a policy being worked out. It is labelled as an illustration, not
  a recorded run, because it is a drawing.
- **The playground** is two columns on a wide screen, with the result staying in view while you edit, and one column on a
  phone. The result shows each condition in the order it was checked: holds, fails, or not reached, and the logo's gate
  beside it lights one bar per condition.
- **Things to touch.**
  - The examples are **cards you can swipe** (CSS scroll-snap; buttons for a mouse; the list below is the same choice).
  - A **slider** moves a balance across the minimum, with a line where the answer flips.
  - **Copy a link** puts the whole policy in the address, and a link opens it exactly. What comes back from an address is
    untrusted, so `share.ts` rebuilds the draft field by field, limits every length and count, and ignores anything else.
- **The proof strip** reads its numbers (70 shared vectors, the contract's unit tests, 120 comparisons, 26 and 29 steps) from
  the committed vectors, evidence and test file when the site is built, each with the label the evidence glossary gives it.
  Nothing in the page is typed in by hand, and `test/meta.test.ts` fails if a number is.
- **Motion** is small and plays only when something changes, not on every keystroke. Where the browser can tie animation to
  scrolling (Chrome and Edge can; others vary), a line fills as you read, the top bar turns to glass, sections rise into place
  and the hero's glow drifts; a change of example is a view transition. All of it sits inside `@supports`, the plain state is
  the finished one, and the page update never depends on a transition running (a timer makes it if one stalls). A person who has asked their system for less motion
  gets none of it, and the proof numbers simply show their value.

## Develop

Building needs Node 22.12 or newer (Astro's requirement, and the SDK's too). The tests also use jsdom, which asks
for 22.22.2, 24.15 or newer; on 22.22.1 npm prints a warning and the tests still passed. CI uses the latest 22, and one job also runs the tests on 22.12.0.

```bash
npm ci                              # from the repository root
npm run dev -w @sorogate/site       # http://localhost:4321/sorogate/
npm test -w @sorogate/site
npm run build -w @sorogate/site     # static files in packages/site/dist
```

The page imports the SDK's source, not its build, so `npm test` and `npm run typecheck` do not need the SDK to be built
first.

## What the tests do not cover

- **Real browsers.** The page test runs in jsdom. It checks behaviour, labels and that typed text is not turned into HTML,
  but not layout, focus order on a real screen, or what a screen reader says. Do those by hand before a release.
- **Colour contrast in context.** The palette is checked pair by pair by `test/contrast.test.ts`, not by measuring the rendered
  page, so text over the hero's gradient and over translucent surfaces is only covered at the gradient's stops.
- **The look in a real browser.** The layout, the sticky result and the motion were checked in one browser (Edge, headless) at
  a desktop width and at 390, 360 and 320 pixels, in both themes, by eye and by measuring that nothing is wider than the
  screen. Nothing tests them: the scroll-driven effects, the view transition and the carousel's swipe are not exercised by
  any test, and nothing was tried in Firefox or Safari.
- **The published page itself.** The workflow tests and builds the site, but nothing checks the live address after a
  deploy. Open it after the first deploy and after any change to `astro.config.mjs`.

## Publishing

`.github/workflows/pages.yml` publishes the site to GitHub Pages at `https://sorogate.github.io/sorogate/`. It runs on a
push to `main` that changes the site, the SDK source, `spec/vectors/evaluate.json`, the lockfile or the workflow itself, and
by hand from the Actions tab. It runs this package's tests first, so a build whose examples no longer match the vectors is
not published. The one-time switch is in the repository settings: Pages, with the source set to "GitHub Actions".

The page sits under `/sorogate` because it is a project page. `site` and `base` in `astro.config.mjs` say so; change
both if the page moves.
