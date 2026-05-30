# Rules

## Cycle

- monthly
- at cycle close, remaining active issue and solution proposals are archived as cycle history
- each new cycle starts with fresh active boards
- days 1–21: issue submission and discussion
- days 22–30: issue voting
- the first cycle is issue-only until a winning issue is published
- after that, the solution board works on the most recent published winning issue from a prior cycle
- the solution board shows the target winning issue at the top so users can inspect the problem being solved

## Proposal Requirements

### Issues
Must be:

- tangible real-world problems
- specific
- relevant to the locale
- meaningfully distinct from existing proposals

### Solutions
Must be:

- actionable
- linked to a specific issue
- written with black-and-white completion criteria
- clear enough that completion can be verified

## Participation Unlock

To submit a proposal in a cycle:
- complete up to 4 required review actions, depending on how many eligible reviewable submissions currently exist

To unlock voting in a cycle:
- complete up to 4 required review actions, depending on how many eligible reviewable submissions currently exist

Each required review action is a real sentiment review: the user must choose Support, Not a Fit, Unclear, or Unsafe / Illegal / Deceptive for the reviewed proposal. On the Issue Board, the Not a Fit choice may be displayed as Pass while keeping the same stored vote value. A bare "mark reviewed" action does not satisfy the process.

Required reviews are shown one at a time, with the least-exposed eligible submission first. If too few submissions meet the normal review-pool rules, the system falls back to the least-exposed active submissions.

The normal Issue and Solution board feeds use the same review-priority ordering in repeating sets: low-exposure, contested/under-reviewed, merge-heavy, and low-rated-but-salvageable candidates, then fallback by least exposure.

Required Reviews is not a normal navigation section after unlock; it exists only as the forced review step before participation opens.

Required review credit cannot be earned on proposals authored by the reviewing user.

## Vote Types

Each user may cast one sentiment vote per proposal:

- Support
- Not a Fit, displayed as Pass on the Issue Board
- Unclear
- Unsafe / Illegal / Deceptive

Each user may also independently cast on active proposals:

- Merge

Each merge vote must select the other active proposal it is targeting.

Merge signaling unlocks after required reviews are complete for that board and may happen during the active cycle before the formal sentiment-voting window.

Archive Board voting supports sentiment votes only. Historical archived proposals are not merged across cycles.

When a merge is executed, the lower total-count proposal merges into the higher total-count proposal. Either side may satisfy the merge threshold, but only merge votes targeting the other proposal in that pair count toward that threshold.

Distinction notes require an active merge relationship and may be submitted only after the source proposal receives enough duplicate signals.

## Live Vote Visibility

During active cycles, public users do not see live vote totals or ratios.

## Moderation

Moderators may:

- archive
- freeze
- merge after review
- resolve appeals

Archived proposals remain viewable and auditable.

Archived proposals return to active status only through appeal or reconsideration outcomes.

Merged proposals and routine cycle-close archives do not use the normal active-restore path in v1.

Archived proposals from prior cycles may be copied into a new submission form, but they must be reviewed and submitted as new proposals for the current cycle.
