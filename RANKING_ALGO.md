# How the AI models are chosen

The AI edit dialog offers a short list of models. That list used to be built by
reading leaderboards by hand, which was defensible on the day it was written and
a little less so every day after. `editor-rs/src/models.rs` derives it instead,
from two public sources; the dialog's **update button** runs that derivation and
stores the result, and the dialog says the date of the last refresh that
succeeded next to the list it produced.

What comes out is an opinion, and this document exists to be disagreed with.

## The two sources, and why each

**Quality — LMArena's image-editing arena**, read through HuggingFace's
datasets-server:

```
https://datasets-server.huggingface.co/rows
  ?dataset=lmarena-ai/leaderboard-dataset
  &config=image_edit&split=latest
```

No key, no quota, no attribution clause, 53 models. It gives a rating, a
confidence interval and — the part that matters most — a **vote count**, running
from a few thousand to over half a million. The plain `/rows` endpoint, not
`/filter`: the split is under a hundred rows, so picking the `overall` category
and sorting by rank happen client-side in two lines — and the day this was
written, `/filter` answered 500 to everything while `/rows` kept working. The
simpler server path is the one that is up.

**Reach and price — OpenRouter's image catalogue.** It is the only side that
knows what a given account can actually use: whether a model is served at all,
whether it takes a reference image (edits) rather than only a prompt
(generates), and what each provider charges for it.

Neither source can do the other's job. The arena has no idea what your key can
reach; the catalogue has no idea which model is any good.

## The algorithm

The rules are two, and short.

**1. Eligibility.** A model has to be reachable by this account, to accept a
reference image, and to be on the board with at least `MIN_VOTES` votes. This is
a gate, not a score.

**2. Matching.** The arena writes `gemini-3.1-flash-image (nano-banana-2)`; the
catalogue writes `google/gemini-3.1-flash-image`. Names are stripped of their
parenthetical and punctuation and must then match **exactly**, with a short
table of hand-written aliases for the cases where no rule would ever bridge the
two.

**3. Price on one scale.** Every price is converted to what a single edit at
1024×1024 costs, from the cheapest provider serving that model.

**4. Frontier** — the three models with the highest **lower bound** of the
rating interval. Not the highest rating: the lower bound is what rewards a model
for being both well rated and well established, and it is the reason a newcomer
with a thousand votes cannot leapfrog on noise. Cost plays no part here.

**5. Economical** — of the models within `MAX_ELO_GAP` elo of the best, and not
already in the frontier list, the three **cheapest**. Quality decides who
qualifies; price decides the order. If fewer than three qualify, fewer are
offered — two good ones beat three with a bad third.

## The refresh, and what survives it

The button calls `models::refresh()`: derive, then store the result as
`models.json` beside the app's settings. One column is **carried over** from
the list being replaced rather than derived, because no board knows it:
`seconds`, the round trip measured on a real edit. A model that survives a
refresh keeps its measurement; a new arrival shows none until a person times
it. When the refresh fails — a board down, a key missing — the stored list
stays exactly as it was and the dialog keeps showing it under its own date: a
refresh that did not happen is not an update.

Before the first refresh the dialog shows the **seed**: `CURATED` in
`editor-rs/src/models.rs`, curated by hand on 2026-08-30 and dated as such -
`CURATED_AT`, which is the date the dialog shows until a refresh replaces it.

## The user's own edits keep it honest

Every plain edit the app performs is also a measurement: the response says
what the call cost, the clock says how long it took, and both are written down
with the size of what came back (`measurements.json`, a rolling last-five per
model). Cost and time scale with size and the user picks the output size edit
by edit, so nothing is normalised to a canonical 1024: the menu shows **the
median of the kept edits** — the column header names it exactly — meaning
*what this model tends to cost and take at the sizes it is actually used on*, one
odd edit never overwriting the story, while the derived fallback, for models
never used here, stays anchored at 1024 because a price list needs one anchor
to be comparable at all.

The measured cost is not a second best. The derived price sees only the output
side of the bill; the measured one is the whole of it — the input image and
the instruction included, which on a real edit came to 39% of GPT Image 2's
total. And it cannot go stale the way a written-down number can, because using
the model is what refreshes it: a vendor's price change walks into the median
within a handful of edits.

The first real edits made the point immediately: GPT Image 2 came back at a
tenth of what the hand-curated list claimed, which had been read off a board
that never said which tier it priced, while MAI-Image 2.5's output tokens
reconciled exactly with the catalogue's per-token rate. Upscales are not recorded: a 2K or 4K render has no business
in the economics of a 1024 edit.

## What the dialog shows

Each model is two lines. The first carries what decides the choice — name,
vendor, **the price of one edit** and **the measured seconds** — and when a
price cannot be put on the common scale the line says so ("no price") instead
of going quietly blank, because the models without a comparable price are also
the expensive ones. The second line is the **arena position**. The bare elo is
gone — the arena's scale drifts between snapshots, so the number rots while the
rank holds — and so are the hand-copied weekly-usage figure and the hand-written
notes, which were written against one snapshot of the board and kept talking on
the next one. Under the list, the footer names the board and **the date it was
read**, which is the one piece of information that keeps every other one
honest.

The frontier group is ordered by rank, the economical group by price — the same
orders the algorithm chose them in.

**OpenAI is the other provider, and it has no board behind it.** No arena
covers the direct API and OpenAI publishes no per-image price, so nothing there
wears a rank or a cost: the menu is the two sane defaults — the newest full
`gpt-image` model and the newest mini, read live from `/v1/models` — and the
rest of the image family sits behind the same search the catalogue gets,
unweighed and said so.

## The cost, which is the part that hides a trap

What is being priced is **one edit**: a 1024×1024 output *plus the reference
image sent in*. Grok bills $0.01 for the input on top of $0.04 for the output —
a quarter of the price of the call, invisible to anyone comparing output prices.

The catalogue bills that output in three different units:

| Unit | Who | Conversion to one 1024×1024 picture |
|---|---|---|
| `image` | Seedream, Grok | the price, as it is |
| `megapixel` | FLUX | × 1.048576 |
| `token` | Gemini, GPT, MAI | × the model's token count for that size |

`$0.03` and `$0.00006` are not two numbers that can be compared, and putting
them in the same column would produce a ranking that means nothing.

Some models carry one base price; some price **only per variant** — Grok's
board reads `low_1k`, `low_2k`, `medium_1k`, `medium_2k`, Seedream keeps a
`high_resolution` entry beside its base one. The base entry wins when there is
one. Otherwise only 1k-sized entries qualify, and among them the tier the arena
actually rated: the board scores `grok-imagine-image-2.0 (low)`, so the price
set beside that rating is `low_1k`, not whichever entry happens to come first
in the payload. A rating from one tier with a price from another would be a
number that means nothing wearing two numbers that meant something.

The token conversion is the one that could be fudged, so it is not. There is a
small table of token counts, and an entry only goes in when the vendor publishes
the number **and** it reconciles with what OpenRouter charges:

> Google's pricing page gives 1120 tokens and $0.067 per 1K image for Gemini 3.1
> Flash. OpenRouter charges $0.00006 per output token. 1120 × 0.00006 = $0.0672.

OpenAI and Microsoft are deliberately absent from that table. OpenAI's pricing
page gives the per-token rate and sends you to a calculator for the count;
Microsoft's MAI publishes no count at all. A number nobody published is a number
this module will not invent, so those models come out with **no price**: they
drop out of the economical ranking and stay eligible for the frontier one,
where cost plays no part.

## What the refresh says out loud

Three failure modes of the derivation are silent, so the refresh keeps them and
the dialog says them out loud:

- **what failed to match**, best-ranked first, with vote counts — so a model
  quietly missing from the list is visible rather than merely absent;
- **which aliases no longer resolve** — each one is a model silently dropped;
- **which models have no comparable price**, and are therefore in one ranking
  and not the other.

The seed itself deserves updating now and then — the refresh needs no pasting,
but a
machine that has never refreshed shows the seed, and the seed should not rot
either.

## Design decisions

Each of these was settled by measurement rather than by argument:

**The board is `image_edit`, not `text_to_image`.** This application edits
pictures, and the dataset carries a separate `image_edit` config with a
materially different order: `grok-imagine-image-2.0` and `mai-image-2.6` sit
far higher at editing than at generating.

**Matching is exact, never partial.** A first pass with prefix matching
produced `gpt-image-1.5-high-fidelity → openai/gpt-image-1`: two different
models. A wrong match does not drop a model from the list — it puts **one
model's rating on another**, and nothing downstream can tell. So a name either
matches exactly after normalisation, or it goes through the alias table, or it
is reported as unmatched. Nothing is guessed.

**A Pareto front and a value score were tried first, and dropped.** On this data
the front collapsed to a single model, so a fallback had to fill the rest, and
the value ordering that came out was the price ordering anyway. A quality gate
plus cheapest-first gives the same answer and can be explained in one sentence.

**The cost is the cost of an edit, at the rated tier.** Pricing a bare
generation would understate it: an editor pays for the reference image too, and
the per-variant listings have to be resolved to the tier the arena rated rather
than to whatever entry comes first.

**There is no `available` flag to check.** The endpoints payload carries
pricing, provider name and supported parameters — nothing else. Checked against
three models; the check was dropped rather than pretended.

**The two lists do not overlap.** Nothing already chosen for the frontier list
is offered again as a bargain. Without this the same model appeared in both,
which narrows the choice the two lists exist to widen.

**Transient failures are retried.** Both boards answer `500` occasionally, and
a run reads about twenty addresses; server errors and timeouts are retried,
while a `401` or a `404` is raised at once, because those mean the same thing
however often they are asked.

**`MIN_VOTES` is reported rather than trusted.** On today's board every one of
the 53 models clears it, so it filters nothing. It is kept for the day that
changes, and the refresh counts how many it excluded — currently zero.

The pricing rules — variant selection, the reference-image charge, the token
table, the refusal to guess — and the refresh mechanics — carry-over, the
store, the fallback to the seed, the OpenAI split — are pinned by tests in
`editor-rs/src/models.rs` itself, with the real Seedream and Grok price lists as
fixtures.

## What it produces

One refresh run, through the same path the button takes:

```
FRONTIER
  #1   openai/gpt-image-2               no price   9s   199,275 votes
  #2   x-ai/grok-imagine-image-2.0      $0.0500         5,937 votes
  #5   microsoft/mai-image-2.5          no price   22s  170,301 votes

ECONOMICAL
  #6   bytedance-seed/seedream-5-0-pro  $0.0480
  #7   x-ai/grok-imagine-image-quality  $0.0600
  #10  google/gemini-3.1-flash-image    $0.0672    10s
```

The seconds shown are the ones carried over from the hand-measured seed; the
models new to the list have none until somebody times them.

## Where it is weak

**Seventeen of fifty-three match.** Most of the misses are correct — OpenRouter
does not serve `reve`, `uni` or `hunyuan`, and `chatgpt-image-latest` is not an
API model at all. But some of them carry half a million votes, and a model this
application could use and is not offering is worth a look before the list is
trusted.

**A model never used here shows its blind spots.** Until the first edit, its
seconds come from the hand-timed seed or nowhere, and a token-priced model
without a published count shows no price at all. Both cure themselves with
use, but the first look at a fresh arrival is the least informed one.

**The thinnest evidence wins a frontier place.** `grok-imagine-image-2.0`
enters on 5,937 votes against 199,000 for the model above it. The lower bound
is doing its job — it is still ranked second on it — but that pick rests on
less than the others.

**The alias table will rot.** It is the only hand-maintained part, and the day
a vendor renames a model it stops resolving. The refresh says that; nothing
enforces it.

**It ranks quality by votes on a public arena.** That is a good signal and it
is not the same as being good at the one edit you are about to make.

**The economical tier promises nothing about the frontier.** The first
measured round showed the top-ranked model to also be the cheapest at the
sizes it was measured on - GPT Image 2 exposes no resolution tiers and stays near $0.02
while everything that scales with size climbs past it. The lower group is the
near-top picked for list price, and its heading claims exactly that much.
