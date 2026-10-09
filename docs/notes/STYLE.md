# How a note is written

The contract for every page under getvela.app/notes (spec 101). The mechanical parts are
enforced by `app-web/getvela.app/src/lib/content/notes.test.ts`, which reads the banned list
below from this file. The rest is for the writer and the reviewer.

## Who reads it

Native English speakers: mostly technical people who run things themselves, arriving from a
search with one question. Notes are English only; there is no other edition. Write the English a
careful native writer would write, not English that reads as translated.

## What a note is

One true thing about Vela, told so that a stranger can check it. Either a reason (why something
is the way it is) or a use (what a setting does, how to pay less, how to add a layer of
security). Never both in one page.

A note exists only if it has something concrete behind it: a number, a date, an issue, a
measured case, a quote, a line of code. If you can't find one, there is no note.

## Shape

What matters is that it reads easily: a reader gets the point in seconds and finishes in a
minute or two without effort. There is no fixed template. What usually works:

- **The answer first.** The `description` (one or two sentences, 160 characters at most) is shown
  under the title and used in search results, so the body can go straight to the reason.
- **Facts, when there are numbers.** `facts:` takes up to four `"Label | value"` pairs, shown as
  a small strip under the answer ("Limit | 27 bytes"). Leave it out when there's nothing to count.
- **Short paragraphs, plain section names** ("What happened", "What Vela does now") where they
  help a reader find their place. A list when items are parallel. No heading for one sentence.
- **Limits, stated plainly.** Say what it doesn't do or doesn't cover, in a sentence where it
  fits. It doesn't need its own section.

Titles: the question a person would type, or the plain statement of the fact; give a `nav:`
label when the title is over 40 characters. Most notes run 150 to 300 words; if one wants a lot
more, it's probably two notes.

## Voice

Write like the bun.sh guides: plain technical prose that explains how a thing works and why, and
stops. The founder's verdict on the first pilot (9 October 2026): it read as machine-written, too
polished, too many quotable lines. The fix is less writing, not better writing.

What bun does, with its own sentences:

- **The product and the reader are the subjects.** "Bun reads the `paths` field in your
  tsconfig.json to re-write import paths." Write "Vela stores…", "you can…". No "I", no "we
  decided", no story told in the first person.
- **One fact per sentence, causes stated plainly.** "These scripts represent a potential security
  risk, as they can execute arbitrary code on your machine." Use "because", "so", "since".
- **Limits are stated flatly.** "Hot reloading doesn't reload the page in your browser." No
  defence, no apology, no "the trade-off" drumroll.
- **Section names are the reader's questions** when a note needs sections: "How is this different
  than setting a variable?"
- **It stops.** At most a closing "See …" pointer. No summary, and no lesson drawn at the end.

History is fine when it explains the design, told as plain fact with a date: "In a test on 28
September 2026, the relay's responses were deliberately dropped. A send landed, but the app showed
Failed." Don't quote rulings or code comments for colour.

Use contractions (doesn't, can't, it's). Use the words the app uses on screen. British spelling, as
the docs mostly use: licence, behaviour, colour. Dates as "26 September 2026". Identifiers in
`code`.

## What reads as machine-written (avoid; the reviewer checks these)

- Rhetoric everywhere. A well-turned line or a short reflection is fine where it genuinely earns
  its place, once, used with restraint, the way a careful person writes (founder, 9 October 2026).
  What reads as a machine is a turn of phrase in every paragraph: "far more than a mis-tap and far
  less than a fight", then "a handle you can't drag is a control that can't act", then a punchy
  last line. If a note has more than one sentence that would look good on a slide, cut back.
- Contrast pairs and reversals: "It's not X, it's Y", "not half a lock but a whole way in".
- Dramatic framing: "the worst failure", "that gap is the one the attack went through", "I learned
  how real that is".
- Rhythm devices: lists of three for cadence, one-line paragraphs for effect, a punchy last line.
- Announcing the structure: "In this note…", "Let's look at…", "Here's the thing".
- A colon reveal in every paragraph ("The result: …").
- Bold for emphasis. Bold is for labels, such as the question that starts a paragraph.
- Hedging that commits to nothing: "can potentially", "may help to".
- More than two em dashes in a note (tested). Use a full stop.
- Chinese or any other non-English text in the prose (tested).

## Facts

- Re-read the cited source when you write. `docs/notes/INVENTORY.md` holds leads, not facts; it
  was a day old the moment it was written.
- `specs/080-site-content-accuracy/claim-ledger.md` wins. Where a note states a ledger fact, use
  the ledger's wording.
- Never describe something unshipped as shipped (081 FR-020).
- `docs/CONTENT-SOURCE-100-CLUES.md`, `docs/requirements/*`, the Expo-era project-takeover docs
  and the README's lower half are stale in places. Use them to find a source, never as one.
- Don't lead with a multiple of the on-chain cost when describing the fee (founder, 22 September
  2026).
- Notes that state the fee formula or the default speed wait for `fix/eth-mainnet-fee`.

## Frontmatter

```yaml
---
title: "Why a wallet name fits nine Chinese characters, but not ten"
nav: "How long a wallet name can be"
description: "A wallet name can be 27 bytes long. That is 27 English letters, but only nine Chinese characters, because each one takes three bytes."
facts:
  - "Limit | 27 bytes"
  - "English | 27 letters"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/mod.rs#L98-L103
related:
  - security-layers/decide-your-keys-first
docs: create-wallet
featured: true   # optional: one of the index's "Start here" cards
draft: true      # renders in `vite dev` only until reviewed
---
```

`sources` are repository paths (the test checks each one exists) or full URLs for other
repositories. Quote any value that contains a colon.

## Banned words and phrases

Matched case-insensitively as whole words. Add to this list when a review catches a new one.

```banned
seamless
seamlessly
robust
leverage
leverages
empower
empowers
elevate
delve
crucial
pivotal
paramount
journey
landscape
game-changer
game-changing
cutting-edge
state-of-the-art
best-in-class
world-class
revolutionary
effortless
effortlessly
harness
testament
tapestry
holistic
synergy
ever-evolving
in today's
it's worth noting
it is worth noting
let's dive
dive into
deep dive
in conclusion
in summary
at the end of the day
rest assured
peace of mind
furthermore
moreover
in essence
simply put
a myriad
here's the thing
needless to say
here's why
that's the point
the point is
make no mistake
to be clear
at its core
```
