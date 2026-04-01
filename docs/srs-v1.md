# SRS v1

## Proposal States

Primary states:

- Draft
- Active
- Emerging
- Ranked
- Merged
- Removed
- Archived

Watch flags:

- Merge-Watch
- High Merge-Watch
- Moderation-Watch
- High Moderation-Watch
- Forced-Review Eligible

## Thresholds

### Emerging
- fewer than 12 non-merge votes

### Ranked
- 12 or more non-merge votes

### Merge-Watch
- 10+ total votes
- merge votes are 20%+ of total votes

### High Merge-Watch
- 20+ total votes
- merge votes are 35%+ of total votes

### Moderation-Watch
Triggered when any of the following are true:
- 8+ total votes and unsafe votes are 20%+ of total votes
- 5+ unsafe votes
- negative votes exceed 8x support, counting 0 support as 1, with at least 10 non-merge votes

### High Moderation-Watch
- unsafe votes are 35%+ of total votes
- or 8+ unsafe votes

## Ranking

Ranked proposals are sorted by:

1. support ratio = support / (support + negative)
2. higher non-merge vote count
3. lower unsafe fraction
4. older submission time

Merge votes do not affect support ratio.

## Forced Review Buckets

### Low-rated but salvageable
- at least 3 non-merge votes
- negative votes are no more than 4x support
- count 0 support as 1

### Contested and under-reviewed
- 6 to 20 non-merge votes
- support ratio between 40% and 60%

### Merge-heavy
- at least 10 total votes
- merge votes are 20%+ of total votes

### Low-exposure general pool
- fewer than 12 non-merge votes
- low review count relative to cycle average

## Forced Review Exclusions

Do not include a proposal in forced review if it is:

- merged
- removed
- under high moderation watch
- beyond the extreme rejection threshold
- below minimum content quality
- already reviewed by that user

## Merge Distinction

When under merge review, the author may attach a distinction note to the relationship between two proposals.

## Appeals

Appeals must include:
- short reason
- distinction or correction argument if relevant

Where possible, a different moderator from the original actor should review the appeal.

## Execution Records

Winning solutions become tracked execution records with:

- linked winning solution
- required resource categories
- completion criteria
- current progress state
- external implementation links
- status updates / evidence notes