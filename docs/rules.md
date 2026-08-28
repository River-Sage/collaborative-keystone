# Rules

## Cycle

- monthly
- throughout the active month, submission, required review, voting, duplicate-link signaling, and discussion can all happen concurrently after the participant satisfies review unlock
- at closeout after the cycle ends, winners are resolved, results are published, remaining active issue and solution proposals are archived as cycle history, and the next fresh cycle opens
- each new cycle starts with fresh active boards
- the first cycle is issue-only until a winning issue is published
- after that, the solution board works on the most recent published winning issue from a prior cycle
- the solution board shows the target winning issue at the top so users can inspect the problem being solved
- live vote totals are hidden during the month; published results remain visible and auditable after closeout

## Proposal Requirements

### Issues
Must be:

- tangible real-world problems
- specific
- relevant to the locale
- meaningfully distinct from existing proposals

When locale names appear in user-facing issue prompts, `World` should read as `the World`. Other locales use their normal display name.

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

Each required review action is a real sentiment review: the user must choose Support, Not a Fit, Unclear, or Unsafe / Illegal / Deceptive for the reviewed proposal. On the Issue Board, the Not a Fit choice may be displayed as Downvote while keeping the same stored vote value. A bare "mark reviewed" action does not satisfy the process.

Required reviews are shown one at a time, with the least-exposed eligible submission first. If too few submissions meet the normal review-pool rules, the system falls back to the least-exposed active submissions.

Required review progress should be shown as the current review number, starting at 1. When fewer than 4 reviews are required, show that smaller available count and how many remain after the current review.

The normal Issue and Solution board feeds use the same review-priority ordering in repeating sets: low-exposure, contested/under-reviewed, merge-heavy, and low-rated-but-salvageable candidates, then fallback by least exposure.

Required Reviews is not a normal navigation section after unlock; it exists only as the forced review step before participation opens.

Required review credit cannot be earned on proposals authored by the reviewing user.

## Vote Types

Each user may cast one sentiment vote per proposal:

- Support
- Not a Fit, displayed as Downvote on the Issue Board
- Unclear
- Unsafe / Illegal / Deceptive

Each user may also independently cast on active proposals:

- Merge

Each merge vote must select the other active proposal it is targeting.

Merge signaling unlocks after required reviews are complete for that board and may happen throughout the active cycle.

Archive Board voting supports sentiment votes only. Historical archived proposals are not merged across cycles.

When a merge is executed, the lower total-count proposal merges into the higher total-count proposal. Either side may satisfy the merge threshold, but only merge votes targeting the other proposal in that pair count toward that threshold.

Distinction notes require an active merge relationship and may be submitted only after the source proposal receives enough duplicate signals.

## Discussion

- discussion belongs to individual submissions, not separate board-wide forums
- each user may post one comment per submission
- authors may comment on their own submissions, labeled only as Author
- comments never expose emails, raw user IDs, public user IDs, usernames, or profiles
- each new comment starts with a like from its author
- users may like or dislike comments, but comment vote counts and ratios are hidden
- comments are sorted by hidden like-to-dislike ratio
- comment votes affect only comment ordering, not proposal ranking or outcomes
- archived submissions keep historical comments visible but do not accept new comments or comment votes in v1

## Live Vote Visibility

During active cycles, authenticated users do not see live vote totals or ratios. Guest browsing of proposal, result, implementation, archive, and merge-relationship content is disabled for now.

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
