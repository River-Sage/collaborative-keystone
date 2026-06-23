# Testing Sanity And Edge Cases

This document maps the local DB test suite to the v1 requirements and expected states.

## Baseline Sanity

Run after a clean reset:

```powershell
Reset-CkDatabaseFull
Test-CkSeedRequirements
```

Or from Python:

```powershell
python .\scripts\dev_db_test_gui.py --sanity
```

Expected baseline:

- one active `world` locale
- active `issue`, `solution`, and `archive` boards
- one active World cycle
- dev accounts are verified and role-correct
- dev accounts have no sessions, auth tokens, reviews, sentiment votes, or duplicate-link votes
- four active demo issues, one archived prior issue winner, and four active demo solutions
- the Solution Board targets the prior published issue winner
- solution proposals include resource categories, completion criteria, and execution tracking entries
- proposal counters match the vote tables
- duplicate-link votes are targeted, active, same-board/same-cycle/same-locale, and backed by active relationships
- two high duplicate-link review scenarios exist without auto-merging
- demo duplicate-link review scenarios have seeded author and moderator notification records
- no active proposal starts in high moderation watch
- appeal, reconsideration, watch flag, and execution facets are clean after full reset
- anti-abuse/trust-review flags and dev-account activity signals are clean after full reset
- each dev account has four eligible issue reviews and four eligible solution reviews available

## Visibility Rules

Expected active-cycle behavior:

- public proposal lists and details do not expose live vote totals
- active public users should not see live support ratios, unsafe fractions, merge percentages, or internal watch labels
- moderators may see only the threshold count signal that triggered a moderator review state
- cycle outcome/result views may expose final counts after resolution
- required-review cards must collect a real sentiment vote without displaying live totals to ordinary users

Niche expectation:

- if a public active-cycle surface needs counts to compute an internal label, compute it server-side or hide the label; do not ship raw counts to public response shapes

## Review Unlock

Baseline expected state:

- `test2@example.com`, `user@example.com`, and `moderator@example.com` each start with `0/4` eligible reviews on issues and solutions
- authored-by-self proposals never count for review credit
- archived, merged, frozen, high-moderation-due, and already-reviewed proposals do not count
- if fewer than four eligible proposals exist, the required count scales down to the eligible count
- if zero eligible proposals exist, unlock should not deadlock first submissions

Useful commands:

```powershell
Reset-CkUserParticipation -Email test2@example.com
Reset-CkUserLoginState -Email test2@example.com
```

Expected after participation reset:

- selected user has no `review_actions`
- selected user has no sentiment votes
- selected user has no duplicate-link votes
- proposal counters are refreshed

## Cycle Phases

Active phase:

- submissions can open after review unlock
- sentiment voting can open after review unlock
- duplicate-link signaling remains review-gated
- submission, review, voting, duplicate-link signaling, and discussion all happen concurrently during the monthly active cycle

Closed phase:

- submissions and voting are closed
- moderator outcome resolution can be tested
- after resolution, remaining active proposals should archive as cycle history and the next cycle should open fresh active boards

Useful commands:

```powershell
Set-CkCyclePhase -Phase active
Set-CkCyclePhase -Phase closed
```

## Duplicate-Link And Merge Scenarios

Baseline high duplicate-link scenarios:

- `DEMO ISSUE: Duplicate clean water access framing` targets `DEMO ISSUE: Clean water access gap`
- `DEMO SOLUTION: Mobile water testing training corps` targets `DEMO SOLUTION: Regional water lab network`

Expected state:

- each duplicate-link vote has a target proposal
- source and target are active
- source and target share board, cycle, and locale
- an active merge relationship exists for the directed pair
- high duplicate-link thresholds do not auto-merge
- moderator merge execution archives the lower-total proposal into the higher-total proposal
- sentiment votes transfer only where the voter has not already voted on the survivor
- conflicting or duplicate transferred votes are audited and discarded

Useful command:

```powershell
Reset-CkMergeFacet
```

Niche failures that should be accepted:

- self-target duplicate link
- nonsensical ID
- target on another board
- target in another cycle or locale
- archived target
- duplicate-link vote before review unlock

## Moderation

Baseline expected state:

- no active demo proposal is in high moderation watch
- moderators should not archive or freeze active proposals before high moderation watch, except separate system-abuse handling outside normal proposal governance
- freeze is represented with an active `frozen_for_review` watch flag, not by changing the proposal primary state

Useful command:

```powershell
Reset-CkModerationFacet
```

Expected after moderation reset:

- appeals are gone
- reconsideration windows are gone
- active watch flags are gone
- moderator actions are gone
- demo proposals return to active/archived baseline states

## Trust Review And Anti-Abuse

Useful commands:

```powershell
Reset-CkTrustFacet
New-CkTrustReviewScenario
```

Expected baseline state:

- no open anti-abuse flags
- no dev-account activity signals after full reset
- seeded demo duplicate-link pairs have merge notification records for the author and moderator audit trail
- moderator-only Trust Review can list open suspicious account/activity flags
- acknowledging or dismissing a flag removes it from the open review queue

Expected seeded scenario:

- one open high-severity `merge_signal_cluster` flag exists for `test2@example.com`
- the flag references `DEMO ISSUE: Duplicate clean water access framing`
- the related submission is `DEMO ISSUE: Clean water access gap`
- the flag includes only coarse seeded network/browser test values, not raw production device data

## Appeals

Useful command:

```powershell
New-CkAppealScenario
```

Expected state:

- an archived proposal named `SCENARIO APPEAL: Archived test2 proposal` exists
- it is owned by `test2@example.com`
- it has archive reason `manual_archive`
- a moderator archive action exists so the audit trail can be exercised
- the author can submit an appeal
- non-authors should not be able to submit the author appeal
- merged proposals and routine cycle-close archives should not use the normal active-restore path in v1

## Reconsideration

Useful command:

```powershell
New-CkReconsiderationScenario
```

Expected state:

- an archived proposal named `SCENARIO RECONSIDERATION: Archived public proposal` exists
- it has archive reason `manual_archive`
- a moderator archive action exists
- a moderator can open a 72-hour reconsideration window
- the proposal can return to active voting during the window
- after the window, it returns to moderator review for restore, re-archive, or freeze
- a proposal should enter reconsideration no more than once per cycle

## Verification And Password Reset

Useful commands:

```powershell
New-CkVerificationScenario -Email test2@example.com -Token dev-verify-test2
New-CkPasswordResetScenario -Email test2@example.com -Token dev-reset-test2
```

Expected verification state:

- selected user becomes unverified
- selected user's sessions are cleared
- old verification tokens are cleared
- one known hashed verification token is inserted

Expected password reset state:

- old reset tokens are cleared
- one known hashed reset token is inserted
- the password does not change until the reset flow consumes the token

Niche note:

- auth rate limits are in memory, not in the database; restart the API process if repeated failed-login testing hits a rate limit

## Execution Tracking

Useful command:

```powershell
Reset-CkExecutionFacet
```

Expected state:

- solution cycle results are cleared
- execution records are cleared
- demo solution proposals return to active
- only active/ranked solution proposals with structured implementation data should create execution records
- execution records remain unique per winning solution and per issue/cycle pair

## Known Follow-Up

The DB sanity checker validates seeded database state. It does not replace API integration tests. The next useful layer is automated endpoint tests for:

- review unlock edge cases
- invalid duplicate-link targets
- moderation threshold enforcement
- appeal authorization
- reconsideration one-per-cycle behavior
- cycle close archival and execution record creation
