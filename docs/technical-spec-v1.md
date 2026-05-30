# Collaborative Keystone v1 — Technical Specification

## 1. Purpose

Collaborative Keystone is a platform for structured democratic problem identification and solution selection.

Version 1 supports three core functions:

1. identifying the most important issue in the active locale
2. selecting solutions for the previously selected issue
3. tracking the execution of winning solutions using structured resource and completion data

The initial locale for version 1 is:

* **World**

This specification defines product rules, internal logic, moderation rules, state transitions, data expectations, and implementation constraints for version 1.

---

## 2. Scope of Version 1

### 2.1 Implemented boards

Version 1 includes the following boards:

* **Issue Board**
* **Solution Board**
* **Archive Board**

### 2.2 Not implemented in version 1

The following concepts are acknowledged but excluded from v1 implementation:

* platform improvement board
* governance/rules board
* native fundraising platform
* native volunteer marketplace
* native skills marketplace
* user editing of submitted proposals
* automatic merging of proposals
* public display of live vote totals or ranking math

### 2.3 Version 1 philosophy

Version 1 should remain as simple as possible while preserving core integrity.

This means:

* hidden live vote mechanics
* minimal but structured proposal schemas
* strict anti-edit rule after submission
* transparent audit trails for moderator actions
* implementation tracking present from day one
* use of third-party external platforms where necessary

---

## 3. Cycle Model

### 3.1 Cycle duration

Each cycle lasts **30 days** total:

* **21 days** submission and discussion
* **9 days** voting

### 3.2 Concurrent board behavior

At any given time:

* the **Issue Board** is used to identify the next winning issue
* the **Solution Board** is used to propose and vote on solutions for the winning issue from the prior issue cycle

During the first cycle, no prior winning issue exists. The Solution Board therefore accepts no submissions and produces no winner until the first issue result has been resolved and published.

### 3.3 Archive Board behavior

The Archive Board is persistent and not tied to a single active phase in the same way as the primary boards.

Archived proposals remain visible there and may receive votes, but Archive Board voting does not directly restore them to active ranking.

---

## 4. Locale Model

### 4.1 Active locale in v1

The only locale in version 1 is:

* **World**

### 4.2 Future locale model

The system should be designed so it can later support:

* world
* country
* state
* county
* city
* other defined locale instances

Locale support should therefore exist as a first-class concept in the data model, even if only one locale is active in v1.

---

## 5. Identity, Onboarding, and Anti-Abuse

### 5.1 Core principle

Version 1 does not attempt to perfectly prove one-human-one-account.

The real objective is to:

* increase friction for fake participation
* reduce low-cost mass bot abuse
* detect suspicious voting patterns
* support moderator response to anomalies

### 5.2 Version 1 minimum anti-abuse stack

Version 1 uses:

* required email verification
* account creation timestamp tracking
* account age thresholds where needed
* rate limiting
* IP heuristics
* device/browser heuristics where legally and technically appropriate
* anomaly detection for unusual review/voting patterns
* moderator review of suspicious accounts or account clusters

### 5.3 Third-party sign-in

Google sign-in or similar may be used for convenience, but delegated login does not count as strong uniqueness verification.

### 5.4 SMS verification

SMS verification is optional for future use.

It should not be required globally in v1 unless usage is tiny and cost remains sustainable.

### 5.5 Participation gating

A newly created account must still satisfy the cycle review unlock rules before submitting or voting in a cycle.

---

## 6. Users and Roles

### 6.1 User roles in v1

Version 1 recognizes:

* **Guest** — may browse public content
* **Registered User** — verified email, may complete review unlocks, submit proposals, vote, and appeal if eligible
* **Moderator** — may act only within specified moderation powers and thresholds

Version 1 does not include an app-level Admin role. Deployment ownership, database maintenance, and moderator appointment are operational responsibilities outside the in-app proposal process and must not bypass proposal, voting, merge, archive, appeal, or reconsideration rules.

### 6.2 Moderator power constraints

Moderators may observe all flagged information before action thresholds are reached.

Moderators may **not** take action on active proposals before **High Moderation-Watch** is reached, except in cases involving system abuse or account-level anti-abuse enforcement outside normal proposal review.

---

## 7. Boards and Their Functional Rules

## 7.1 Issue Board

Purpose:

* propose and evaluate tangible real-world problems relevant to the active locale
* select a single winning issue for the cycle

Inputs:

* issue submissions
* discussion/review signals
* user votes
* merge signals
* moderator actions when threshold conditions are met

Output:

* one winning issue per issue cycle

## 7.2 Solution Board

Purpose:

* propose and evaluate actionable solutions for the issue that won the previous issue cycle
* select a winning solution or winning solutions, depending on future governance rules

Version 1 assumes a single winning solution unless changed later.

Inputs:

* solution submissions tied to the most recent published winning issue from a prior cycle
* structured resource requirements and completion criteria supplied at submission time
* user votes
* merge signals
* moderator actions when threshold conditions are met

User interface:

* the Solution Board must pin a concise reference to the target winning issue above the solution list
* the pinned issue reference should show the issue title and make clear it is the problem currently being solved
* selecting the pinned issue opens it in the detail pane with the same "problem being solved" context visible

Output:

* one winning solution that becomes an implementation record

## 7.3 Archive Board

Purpose:

* preserve archived proposals visibly and audibly rather than deleting them
* allow additional community signal to accrue
* support appeal, reconsideration, and auditability

Archived proposals:

* remain viewable
* may continue receiving votes
* do not automatically re-enter main active ranking
* may return through the reconsideration process only

---

## 8. Proposal Types and Required Fields

## 8.1 General proposal rules

Each proposal must belong to exactly:

* one board
* one cycle
* one locale
* one author account

A proposal cannot be edited by the author after submission in version 1.

## 8.2 Issue proposal requirements

An issue proposal must satisfy the following minimum quality rules:

* describes a tangible real-world problem
* is specific enough to be distinguishable from general outrage, slogans, or vibes
* is relevant to the active locale
* is understandable as written
* is not purely rhetorical
* is not a duplicate in substance of an existing proposal
* is not itself a solution proposal

### Required fields for issue proposals

* **Title**
* **Problem Description**
* **Affected People or Scope**
* **Why It Matters**

## 8.3 Solution proposal requirements

A solution proposal must satisfy the following minimum quality rules:

* is actionable
* is submitted under the issue it proposes to solve
* explains why the proposed action would solve or materially reduce the target issue
* contains completion criteria with a black-and-white end goal
* contains required resource categories
* contains required resource entries at submission time
* is understandable as written
* is not purely aspirational or awareness-only
* is not a duplicate in substance of an existing proposal

### Required fields for solution proposals

* **Title**
* **Action Description**
* **Why This Solves It**
* **Required Resource Categories**
* **Completion Criteria**
* **Required Resource Entries**

No separate field is required for “What issue does this solve?” because the solution is submitted under a specific issue.

No separate field is required for “How will we know this is done?” because this is already covered by completion criteria.

### 8.4 Input size limits

Version 1 uses bounded proposal inputs to keep submissions reviewable and prevent oversized payloads:

* Proposal titles are capped at 120 characters.
* Long proposal descriptions, action descriptions, notes, appeals, and moderation explanations are capped at 2,000 characters.
* Affected people or scope is capped at 500 characters.
* Solution problem-fit explanations are capped at 1,000 characters.
* External implementation links are capped at 2,048 characters.

---

## 9. Implementation Tracking Model for Solutions

### 9.1 Core rule

Every solution proposal must include implementation structure at the time of submission.

The site must not wait until after a solution wins to decide what resources and completion criteria must be tracked.

The site may wait until after a solution wins to attach live tracking sources, external links, acquired amounts, and proof notes.

### 9.2 Proposal-time resource requirements

Each solution submission must include one or more required resource entries. Each entry must support at minimum:

* **Resource Category**
* **Target Amount**
* **Target Unit**
* **Target Needed** (derived or displayed as amount plus unit)

Each solution submission may include up to **64** required resource entries. Resource amount and unit fields are each capped at 64 characters.

Proposal authors do not select a tracking method and do not submit external tracking links for resources in v1.

### 9.3 Implementation resource tracking requirements

When a winning solution becomes an implementation record, each resource entry must support at minimum:

* **Current Acquired Amount**
* **External Coordination/Acquisition Link**
* **Status / Proof Note**

In v1, implementation tracking links are added or changed by moderators after a solution wins.

### 9.4 Supported resource categories in v1

At minimum, the system should support:

* money
* labor / manpower
* skills / trades
* materials
* equipment
* logistics / transport
* organizational support
* other

### 9.5 Completion criteria model

Each solution must contain one or more completion criteria items.

Each solution may include up to **8** completion criteria items.

Each completion criterion should support at minimum:

* **Criterion Description**
* **Completion Status**
* **Evidence / Proof Note**
* **Timestamp of last status update**

Completion criterion descriptions are submitted with the solution proposal. Criterion status, evidence, and update timestamps become implementation-tracking fields after the solution wins.

Completion criterion descriptions are capped at 240 characters. Evidence and proof notes are capped at 2,000 characters.

### 9.6 Third-party execution support

Version 1 may rely on third-party platforms for actual collection or coordination of resources.

Examples include:

* fundraising platforms
* volunteer coordination platforms
* external sign-up tools
* logistics / operations tools

Collaborative Keystone remains the truth and tracking layer, not necessarily the execution engine itself.

Third-party links belong to implementation records, not ordinary proposal submission. This prevents the live Solution Board from becoming a solicitation surface while still allowing real-world execution to use external tools after a solution is selected.

---

## 10. Proposal Submission Restrictions

### 10.1 No post-submission editing in v1

Once a proposal is submitted, the author cannot edit it.

Rationale:

* simplifies implementation
* eliminates vote-reset edge cases
* eliminates minor-vs-material edit disputes
* preserves integrity of what was originally voted on

### 10.2 Replacement by resubmission

If an author wants to materially change a proposal, they must submit a new proposal rather than editing the old one.

The old proposal remains part of cycle history unless archived or merged.

---

## 11. Voting Model

### 11.1 Sentiment vote categories

Each user may cast exactly one sentiment vote per proposal:

* **Support**
* **Not a Fit**
* **Unclear**
* **Unsafe / Illegal / Deceptive**

The Issue Board may display the **Not a Fit** sentiment as **Pass** in the user interface while preserving the internal `not_a_fit` vote value and count.

These are mutually exclusive.

### 11.2 Merge vote

Each user may also independently cast exactly one merge vote per active proposal:

* **Merge**

Each merge vote must identify the other active proposal it is targeting. Untargeted merge votes are not valid in v1 because merge thresholds are pair-specific.

Merge signaling is available on active proposals after the participant has completed the required review unlock for that board. It does not need to wait for the formal sentiment-voting window.

This means a user may cast:

* one sentiment vote
* plus one merge vote

### 11.3 Stored vote counts

Each proposal must store at least the following counters:

* support_count
* not_a_fit_count
* unclear_count
* unsafe_count
* merge_count

### 11.4 Derived values

Internal derived values:

* **negative_count = not_a_fit_count + unclear_count + unsafe_count**
* **non_merge_count = support_count + negative_count**
* **total_count = support_count + negative_count + merge_count**

These values are internal and should not be shown during the live cycle.

---

## 12. Vote Visibility Rules

### 12.1 During active cycle

Users must not see:

* raw vote totals
* support ratios
* negative ratios
* merge percentages
* moderation percentages
* internal proposal states such as Emerging, Ranked, Merge-Watch, Moderation-Watch

### 12.2 During active cycle, visible content may include

* proposal content
* linked merge-related proposals
* author distinction notes on merge relationships, if present
* archive-board location where applicable
* implementation/execution content for solution proposals

### 12.3 After cycle completion

The platform may publish final cycle results, including vote totals and outcome data, after the voting period has closed.

---

## 13. Review Unlock Mechanic

### 13.1 Purpose

The required review mechanic exists to:

* force some exposure to under-seen proposals
* reduce pure early-visibility lock-in
* improve distribution of user attention
* create minimum engagement before participation rights unlock

### 13.2 Submission unlock

To submit a proposal in a given cycle, the user must complete a number of required review actions equal to:

* **the lesser of 4 or the current number of eligible reviewable submissions in that board/cycle**

Examples:

* if there are 0 existing eligible reviewable submissions, the requirement is 0
* if there is 1 existing eligible reviewable submission, the requirement is 1
* if there are 2 existing eligible reviewable submissions, the requirement is 2
* if there are 3 existing eligible reviewable submissions, the requirement is 3
* if there are 4 or more existing eligible reviewable submissions, the requirement is 4

### 13.3 Voting unlock

To unlock voting in a given cycle, the user must complete a number of required review actions equal to:

* **the lesser of 4 or the current number of eligible reviewable submissions in that board/cycle**, once

After this unlock is achieved, the user may vote freely for the remainder of that cycle.

If no eligible reviewable submissions yet exist, the requirement is 0 until they do.

After this unlock is achieved, the user may vote freely for the remainder of that cycle.

### 13.4 Presentation order

The user interface must present required reviews one at a time.

The next required review should prioritize the lowest-exposure eligible proposal first. After the low-exposure pool is exhausted, the selector may use the remaining required-review buckets.

The user should not see a stack of multiple required-review cards while locked.

Required review is an internal forced state, not a persistent user navigation section. After the unlock is complete, the user should not be able to reopen Required Reviews as a normal board section.

The normal Issue and Solution board feeds should reuse the required-review priority buckets as their default ordering, repeating the four-slot priority pattern across the full list rather than using a separate feed tab.

### 13.4.1 Review action contents

A required review action must require the participant to cast one of the normal sentiment choices on the reviewed proposal:

* Support
* Not a Fit
* Unclear
* Unsafe / Illegal / Deceptive

The Issue Board may display Not a Fit as Pass in required-review and voting controls while preserving the internal `not_a_fit` vote value.

The interface must not allow a bare acknowledgement such as "mark reviewed" to satisfy the review-unlock requirement.

### 13.5 Deadlock prevention at cycle start

The review-unlock system must never prevent the first submissions of a cycle.

If the number of eligible reviewable submissions is below the standard review requirement of 4, the unlock requirement automatically scales down to match the number currently available.

This rule applies independently per board and per cycle.

---

## 14. Required Review Pool Selection

A proposal is eligible for the required-review pool only if it is active, not archived, not merged, not authored by that specific user, not already reviewed by that specific user, and not excluded by other rules below.

A proposal authored by the reviewing user cannot count toward that user's required-review credit.

### 14.1 Bucket A — low-rated but salvageable

A proposal qualifies when:

* non_merge_count >= 3
* negative_count <= 4 × support_count
* if support_count = 0, treat support_count as 1 for this ratio check

### 14.2 Bucket B — contested and under-reviewed

A proposal qualifies when:

* non_merge_count between 6 and 20 inclusive
* support ratio between 40% and 60%

### 14.3 Bucket C — merge-heavy

A proposal qualifies when:

* total_count >= 10
* merge_count / total_count >= 20%

### 14.4 Bucket D — low-exposure general pool

A proposal qualifies when:

* total interaction count < 12
* review count is low relative to cycle average
* no exclusion rule applies

### 14.5 Required-review exclusion rules

A proposal is excluded from the required-review pool if any of the following are true:

* archived
* merged
* authored by the reviewing user
* already reviewed by that user
* under High Moderation-Watch / frozen for review
* negative_count > 8 × support_count, with support floor of 1
* fails minimum quality gate

### 14.6 Review-pool fallback behavior

If fewer than the required number of proposals are available from Buckets A through D, the system must fall back to other active proposals in the same board/cycle.

Fallback proposals should be selected by:

1. lowest total review/vote exposure first
2. then lowest non_merge_count
3. then oldest submission time

This fallback exists only to satisfy the unlock mechanic when the normal bucket-based pool is too small.

### 14.7 Fallback constraints

The fallback pool must still exclude proposals that are:

* archived
* merged
* authored by the reviewing user
* already reviewed by that user
* under High Moderation-Watch / frozen for review
* below minimum quality threshold

---

## 15. Internal Proposal State Model

The following states are internal system states unless published after cycle close.

### 15.1 Primary states

A proposal may have one primary state:

* Draft
* Active
* Emerging
* Ranked
* Archived
* Merged
* Removed (reserved for exceptional legal, safety, or system-integrity cases outside ordinary proposal governance)

### 15.2 Watch flags

A proposal may also have zero or more watch flags:

* Merge-Watch
* High Merge-Watch
* Moderation-Watch
* High Moderation-Watch
* Eligible for Reconsideration
* Frozen for Review

### 15.3 State philosophy

A primary state and watch flags must be modeled separately.

This prevents a proposal from losing its main lifecycle position just because it also enters a monitoring condition.

---

## 16. Emerging and Ranked Logic

### 16.1 Emerging threshold

A proposal is considered **Emerging** when:

* non_merge_count < 12

Emerging proposals remain active and visible but do not participate in main ranked ordering.

### 16.2 Ranked threshold

A proposal becomes **Ranked** when:

* non_merge_count >= 12
* proposal is not archived
* proposal is not merged
* proposal is not frozen by moderator process

### 16.3 Ranked ordering

Ranked proposals are ordered internally by:

**support_count / (support_count + negative_count)**

Merge votes do not affect ranked ordering.

### 16.4 Ranked ordering tiebreakers

If two proposals have the same support ratio, sort by:

1. higher non_merge_count
2. lower unsafe fraction
3. older submission time

---

## 17. Merge-Watch Logic

### 17.1 Merge-Watch threshold

A proposal enters **Merge-Watch** when:

* total_count >= 10
* merge_count / total_count >= 20%

### 17.2 Pair-specific High Merge-Watch threshold

A proposal pair reaches **High Merge-Watch** when either directed side of the pair meets both conditions:

* total_count >= 20
* merge votes on that proposal targeting the other proposal / total_count >= 35%

It is not enough for a proposal to have merge votes aimed at unrelated proposals.

### 17.3 Consequences of Merge-Watch

When a proposal enters Merge-Watch:

* author is notified
* moderators are notified
* one or more related proposals may be linked in the system
* author may submit one distinction note per merge relationship

A user merge signal creates or reactivates the explicit merge relationship between the source proposal and selected target proposal.

### 17.4 No automatic merge in v1

No proposal is automatically merged in version 1.

All merges require moderator action after pair-specific threshold conditions are met.

When a merge is executed, the proposal with the lower total vote count is archived into the proposal with the higher total vote count. This direction does not depend on which side met the pair-specific High Merge-Watch threshold.

---

## 18. Merge Relationship Model and UX

### 18.1 Merge relationship storage

The system must support explicit relationships between proposals when merge signaling indicates possible duplication.

Each relationship should support at minimum:

* source proposal id
* related proposal id
* relationship creation timestamp
* active/inactive relationship state
* author distinction note, if present

### 18.2 User-visible merge relationship behavior

When viewing a proposal, the user may see:

* linked proposals that have been connected through merge signaling
* any distinction note attached to that relationship

The user may not see:

* merge vote counts
* merge percentages
* internal merge-watch thresholds

### 18.3 Distinction note behavior

The author of a proposal in Merge-Watch may submit one structured distinction note to explain why the proposal should not be merged.

Suggested structured fields:

* **Difference Type**

  * different scope
  * different cause
  * different affected group
  * different implementation
  * different completion criteria
  * other
* **Explanation**

### 18.4 Distinction note visibility

Distinction notes are visible:

* to moderators during merge review
* on the relationship between the linked proposals
* to users viewing that relationship

---

## 19. Moderation-Watch Logic

### 19.1 Moderation-Watch threshold

A proposal enters **Moderation-Watch** if any of the following are true:

* total_count >= 8 and unsafe_count / total_count >= 20%
* unsafe_count >= 5
* non_merge_count >= 10 and negative_count > 8 × support_count, with support floor of 1

### 19.2 High Moderation-Watch threshold

A proposal enters **High Moderation-Watch** if any of the following are true:

* unsafe_count / total_count >= 35%
* unsafe_count >= 8

### 19.3 Moderator action threshold rule

Moderators may not archive, freeze, or otherwise moderate active proposal content until High Moderation-Watch has been reached.

Merge actions are governed by the separate High Merge-Watch threshold and require an explicit merge relationship between the proposals.

Before that, moderators may observe flags and queue data only.

### 19.4 Consequences of High Moderation-Watch

When High Moderation-Watch is reached, moderators may:

* archive the proposal
* freeze the proposal pending review
* leave the proposal active after review
* record a moderator note

---

## 20. Moderation Powers

In version 1, moderators may:

* archive a proposal
* merge a proposal into another
* freeze a proposal pending review
* unfreeze a proposal
* process archive appeals
* process reconsideration windows
* record moderator notes
* review anti-abuse anomalies

Archived proposals may return to active status only through an appeal outcome or reconsideration outcome. There is no direct moderator unarchive action in v1.

Moderators may not ordinarily hard-delete proposals in v1.

---

## 21. Archive Board and Archive Rules

### 21.1 Archive Board purpose

The Archive Board exists so proposals are not silently erased.

Archived proposals remain:

* visible
* auditable
* voteable
* eligible for appeal and reconsideration when their archive reason allows active restoration in v1

At cycle close, every proposal from the completed cycle that has not already been archived or merged is moved to the Archive Board. This is a normal lifecycle transition, not a punitive moderation action.

Completed-cycle proposals do not carry forward as active proposals in the next cycle. If a participant wants to bring forward an issue or solution from an earlier cycle, they must submit a new proposal for the new cycle. The interface may support this by letting a participant copy an archived proposal into the relevant submission form, where it must be reviewed and submitted as a new proposal.

### 21.2 Archive reasons

A proposal may be archived when, after reaching the required moderation threshold, a moderator determines it should be removed from the active board for reasons such as:

* duplication
* unsafe / illegal / deceptive content
* spam / abuse
* not relevant to the board
* failure to meet minimum quality threshold
* superseded by merge or other cycle logic
* routine cycle close

### 21.3 Archive Board voting behavior

Sentiment votes may continue to accrue on the Archive Board.

However, Archive Board votes do not directly reinsert a proposal into active ranking.

Those votes are considered only in moderator reconsideration and appeal workflows.

Archive Board voting is persistent across cycles. A proposal from a completed cycle may remain visible and voteable in the Archive Board, but those votes do not affect the active ranking of a later cycle unless the proposal is newly re-submitted.

Merge signaling is an active-cycle relationship mechanic only. Archived historical records cannot receive new merge votes and are not merged across cycles.

Merged proposals and routine cycle-close archives remain visible and auditable, but they do not use the normal appeal/reconsideration restore path in v1. Cycle-close archives may be copied into new submissions. Merged proposals require a separate merge-reversal process if that is ever introduced.

---

## 22. Reconsideration and De-Archive Flow

### 22.1 Reconsideration trigger

If an eligible archived proposal is flagged by a different moderator as potentially valid or unfairly archived, the proposal may be marked:

* **Eligible for Reconsideration**

### 22.2 Reconsideration window

When marked Eligible for Reconsideration:

* the proposal returns to the active board
* the proposal is available for fresh voting for a fixed **72-hour reconsideration window**
* after the window closes, the proposal automatically returns to moderator review

### 22.3 Reconsideration review outcome

After the reconsideration window, a moderator, preferably not the original archiver, must choose one of:

* restore the proposal to active status
* return the proposal to the Archive Board
* freeze the proposal for further review

### 22.4 Reconsideration frequency limit

A proposal may enter reconsideration no more than **once per cycle**.

---

## 23. Appeals

### 23.1 Author appeal right

If an eligible proposal is archived, the author may submit an appeal.

### 23.2 Appeal contents

An appeal must include:

* short reason for appeal
* any clarification or distinction argument the author wishes to make

### 23.3 Appeal handling

Appeals enter a moderator review queue.

Where possible, a moderator different from the original archiver should review the appeal.

### 23.4 Appeal outcome logging

Appeal outcomes must be logged in the audit trail.

---

## 24. Audit Trail Requirements

The system must permanently log major actions affecting proposal state and governance legitimacy.

### 24.1 Actions that must be logged

* archive
* unarchive
* freeze
* unfreeze
* merge
* merge reversal, if ever allowed
* reconsideration window start
* reconsideration window end
* appeal submission
* appeal outcome
* moderator notes tied to actions

### 24.2 Required logged data

For each action, store at minimum:

* action id
* action type
* proposal id
* related proposal id where applicable
* moderator id or system actor id
* timestamp
* action reason
* public/internal note text
* relevant state snapshot or reference

### 24.3 Merge-specific logging

Merge events should additionally store:

* source proposal snapshot reference
* target proposal snapshot reference
* vote counts at merge time
* whether author distinction note existed
* whether author was notified

---

## 25. Discussion of Public Labels

Version 1 should avoid exposing internal status labels such as Emerging, Ranked, Merge-Watch, or Moderation-Watch during the live cycle.

The only visible board-level distinction necessary in normal usage is whether a proposal is on:

* the active board
* the Archive Board

This helps minimize strategic behavior, vote gaming, and social inference from labels.

---

## 26. Winning Proposal Resolution

### 26.1 Issue outcome

At cycle close, the top valid issue proposal is selected as the winning issue according to the internal ranking rules in effect at close.

### 26.2 Solution outcome

At cycle close, the top valid solution proposal under the active issue is selected as the winning solution according to the internal ranking rules in effect at close.

If there is no published winning issue from a prior cycle, the Solution Board has no valid target and resolves with no solution winner.

### 26.3 Winning solution transition

The winning solution must become an implementation tracking record without requiring re-entry of structured implementation data.

This is possible because the required execution fields already exist at submission time.

### 26.4 Cycle close archival

After cycle results are resolved and published, the system archives remaining active issue and solution proposals from that cycle with a cycle-close archive reason.

The next cycle starts with fresh active boards. Proposals from earlier cycles remain available in the Archive Board as historical records and may be used as source material for new submissions, but they are not themselves active candidates in the new cycle.

---

## 27. Implementation Constraints for v1

### 27.1 Simplicity constraints

To preserve feasibility in version 1:

* do not implement user editing after submission
* do not implement public live vote totals
* do not implement public live ranking labels
* do not implement automatic merge
* do not implement native fundraising from scratch
* do not implement native labor/skill marketplace from scratch
* do not expose proposal-time tracking-method selection for solution resources

### 27.2 Required complexity that must remain

Version 1 must still include:

* hidden but rigorous vote logic
* merge relationship tracking
* distinction note support
* archive and reconsideration logic
* audit trail
* structured implementation tracking for solution proposals
* anti-abuse gating and anomaly awareness

---

## 28. Data Model Summary (Conceptual)

The system must conceptually support at least the following entities:

* User
* Role
* Locale
* Cycle
* Board
* Proposal
* ProposalVoteSentiment
* ProposalVoteMerge
* ProposalRelationshipMerge
* ProposalDistinctionNote
* ModeratorAction
* Appeal
* ReconsiderationWindow
* ImplementationRecord
* ImplementationResourceEntry
* CompletionCriterion
* StatusUpdate / ProofNote

Exact schema design may normalize or combine some of these, but the concept boundaries must remain supported.

---

## 29. Non-Functional Requirements

Version 1 should prioritize:

* auditability
* deterministic rule application
* low operational cost
* low moderation ambiguity where possible
* hidden live-score surfaces
* ease of later expansion into more locales and governance boards

Version 1 should avoid:

* unnecessary moderation discretion before threshold triggers
* hidden destructive actions
* user confusion about whether a proposal changed after voting
* attempting to build all external coordination tools natively
