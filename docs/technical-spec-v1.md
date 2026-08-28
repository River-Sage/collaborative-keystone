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

Each cycle lasts one **UTC calendar month**.

During the active cycle window, submission, required review, voting, merge signaling, and discussion may all happen concurrently after the participant satisfies the relevant review-unlock requirement.

At the end of the calendar month, the cycle closes. During closeout, winners are resolved and published, remaining active proposals are archived as cycle history, and the next cycle starts with fresh active boards. The system is a continuous loop of cycles; the only expected gap between cycles is the short operational closeout period needed to resolve and publish results.

For v1, cycle boundaries are anchored to UTC month boundaries: `00:00:00 UTC` on the first day of a month through `00:00:00 UTC` on the first day of the next month. A fresh deployment that starts mid-month joins the current UTC calendar month instead of opening a rolling 30-day launch cycle. Future locale deployments may add locale-specific timezone boundaries if the project decides that local civic calendars are more appropriate for non-global instances.

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

User-facing sentence copy that names the active locale should render **World** as **the World**. Other locale names should be rendered as their normal display label without automatically adding **the**.

Primary in-app brand copy should prepend the active locale display name to Keystone, such as **World Keystone**, **Castle Rock Keystone**, or **Douglas County Keystone**. This is brand copy, so it should use **World Keystone**, not **the World Keystone**.

### 4.3 Localized deployment model

The project should support a future model where a new locale instance can be spun up from the public repository with a small, documented configuration change rather than code edits.

A localized deployment should be able to define at minimum:

* locale slug
* locale display name
* public web origin
* public API origin
* contact / operator identity
* whether the deployment is official, authorized, or community-operated

The default v1 deployment remains the World locale, but locale configuration must not be hardcoded so deeply that a county, city, state, country, or other community cannot reasonably launch its own instance later.

### 4.4 Global locale access point

Users should access locale-specific Keystone instances through the central global Keystone site.

The global site should act as the trusted entry point, locale directory, and provenance registry. A user who wants to reach Castle Rock Keystone, Douglas County Keystone, or another locale should begin at the global site and choose or search for that locale there.

The global site may route, link, deep-link, or eventually proxy users into a locale deployment, but the user-facing trust path starts from the canonical global site. Direct locale URLs may exist for operations, hosting, and deep links, but they should not be the primary discovery or trust mechanism.

The global locale registry should eventually include:

* locale slug
* locale display name
* locale type
* public web origin
* public API origin
* operator/contact identity
* deployment status: canonical, official, authorized, verified, stale, warning, suspended, compromised, abandoned, community, unverified, or development
* latest verified release identifier
* provenance verification status

Locale deployments that are not listed in the global registry should be treated as unverified community deployments.

Current v1 implementation:

* Each API process runs as one configured locale, identified by `CK_LOCALE_SLUG`, `CK_LOCALE_NAME`, and `CK_LOCALE_TYPE`.
* Startup must create or update the configured locale row and open that locale's active cycle if no active cycle exists.
* Proposal, review, voting, archive, outcome, appeal, reconsideration, discussion, and implementation endpoints must scope user-visible and moderator-visible records to the configured locale.
* The World deployment may expose additional locale registry entries through `CK_LOCALE_REGISTRY_JSON` until a signed registry service exists.
* The web UI may show a compact locale directory when the registry contains more than one locale with a web origin.
* Locale instances must be launchable by environment configuration and helper scripts, not source-code edits.

### 4.5 Brand portability

Localized deployments should preserve the Keystone product identity without confusing users about official status.

The normal in-app brand pattern is:

* **{Locale Display Name} Keystone**

Examples:

* **World Keystone**
* **Castle Rock Keystone**
* **Douglas County Keystone**

Community deployments may use the Keystone software under the project license, but they must clearly identify their locale, operator, source repository, and whether they are an official or community deployment. They must not imply they are the central official instance unless explicitly authorized.

### 4.6 Product and distribution layers

The project should distinguish the software, the canonical service, and local deployable instances as related but separate products.

Required planning layers:

* **Keystone Core** - the public source code, requirements, migrations, and documentation released under the repository software license.
* **Global Keystone** - the canonical hosted service and trusted front door for locale discovery.
* **Locale Keystone Distribution** - an official signed release package generated from Keystone Core to make a new locale easy to run.
* **Locale Keystone Instance** - a running locale deployment with its own database, domain/origin configuration, instance secrets, moderators, audit trail, and implementation records.
* **Community Fork** - any modified or independently distributed version that is not verified as an official release and not authorized as an official locale.

The localized distributable should be different from the main global site by configuration, registry status, and operator identity, not by hidden rule changes. It should be possible to verify that a locale instance is running an official release with only allowed locale/environment configuration changes.

This distinction lets the public repository remain transparent while the official global instance remains verifiable and while local communities can run their own properly labeled Keystone instances.

### 4.7 Signed distributable releases and instance secrets

Locale deployments should eventually be distributed as signed release artifacts or signed container images with a matching manifest.

The release manifest should identify:

* release identifier
* source commit SHA
* artifact digests
* database migration set digest
* supported runtime/spec version
* expected configuration schema version
* official release signature

The locale configuration manifest should identify non-secret deployment facts:

* locale slug
* locale display name
* locale type
* web origin
* API origin
* operator/contact identity
* deployment status
* public instance verification key
* release identifier
* configuration digest

World registry configuration may be supplied as `CK_LOCALE_REGISTRY_JSON` in early deployments. This configuration is not a substitute for a signed registry authority. It is a development and bootstrap bridge that lets the canonical global site publish locale entries while the signing/check-in service is still being built.

Every running instance must publicly expose source/license information and build provenance metadata. In v1 this is implemented as:

* `/source-info`
* `/.well-known/keystone-build.json`
* `/.well-known/keystone-locales.json`

The normal web UI should present this as a plain-language **Source & Trust** surface instead of exposing raw registry/status terminology as primary navigation. The Source & Trust surface should explain whether the user is on the official global site or another Keystone deployment, then offer technical links for source code, license, build details, and locale data.

Secrets must not be committed to the repository or embedded in public artifacts. Instance-specific private material belongs in environment variables, a secret manager, or an operator-controlled secret file that is excluded from source control.

Examples of private instance secrets:

* database credentials
* session cookie signing/encryption secret
* CSRF or token signing secret, if separated
* mail credentials
* instance signing private key
* backup encryption key
* one-time bootstrap moderator token

Cryptographic methods, manifest formats, and verification code should be public. Private keys and deployment secrets should not be public.

Encryption is useful for secrets, backups, operator handoff bundles, and protecting private runtime configuration. It should not be used as a promise that the open-source application code is hidden or unmodifiable. Under the AGPL/open-source model, modified deployments are allowed, but they must be distinguishable from signed official releases.

### 4.8 First moderator bootstrap

A fresh locale deployment needs a safe way to create the first moderator.

On first run, if no moderator exists, the locale instance may expose a bootstrap flow that requires a one-time bootstrap token or local-console command. After the first moderator is created:

* the bootstrap token must be invalidated
* the bootstrap route/command must refuse further use
* the action must be written to the audit trail
* the created moderator should be labeled as the initial locale moderator
* the instance should surface whether bootstrap is complete in its health/provenance metadata

The first moderator does not become an owner of the software or the brand. They become the initial moderator-steward for that locale instance, subject to the same moderation, audit, and implementation-tracking limits as other moderators.

The current v1 HTTP bootstrap implementation is `POST /bootstrap/first-moderator`. It requires `CK_BOOTSTRAP_MODERATOR_TOKEN`, a 32+ character token supplied in the request body, and refuses all future bootstrap attempts after a verified moderator exists. The bootstrap action must be recorded in deployment audit events and exposed as completed in build/provenance metadata.

### 4.9 Canonical instance and build provenance

There should be one central canonical instance operated by the project owner. That instance is the official reference deployment.

The canonical instance should eventually publish a machine-readable provenance manifest, exposed from a stable public path such as `/.well-known/keystone-build.json`. The manifest should include at minimum:

* source repository URL
* git commit SHA
* build timestamp
* build environment identifier
* deployment / registry status
* trust tier
* web artifact digest
* API artifact digest
* database migration set digest
* public release identifier
* signature, Sigstore bundle, or equivalent official attestation over the manifest

A hash alone is not enough to prove official integrity because anyone can hash a modified build. The canonical instance should use cryptographic signing so users, auditors, and future local deployments can verify that a published build matches an official release signed by the project authority.

The target signing path is Sigstore Cosign with project-controlled CI identity and SLSA-style provenance. If the project later uses keyful signing through KMS or hardware custody, the public verification key should be published in the repository and from an official DNS or `/.well-known/` location. If verification material disagrees, the UI or deployment tooling should treat verification as failed.

### 4.10 Tamper-evident official deployment

The long-term official deployment should be hardened so the public can tell whether it is running the expected software and whether important records have been altered unexpectedly.

This does not mean hiding the requirements or source code. Requirements remain public. The goal is an operationally hardened official instance with:

* signed release artifacts
* signed release manifests before public locale distribution is encouraged
* reproducible builds as a later higher trust tier where feasible
* immutable deployment artifacts
* restricted production access
* tamper-evident audit logs
* database backups with integrity checks
* visible official/community deployment status

Forks can modify the open-source code, but they should be visibly distinct from the canonical deployment unless they can prove they are running an official signed release with only allowed locale/environment configuration changes.

### 4.11 Licensing and brand separation requirements

The repository software is licensed separately from the Collaborative Keystone name, logo, visual identity, and official project branding.

Engineering and deployment tooling must preserve that separation:

* AGPL source availability must not be blocked by encryption, packaging, or appliance-style deployment.
* Every running web UI should provide a visible Source & Trust entry point from public/login surfaces and the Settings view. That surface should offer Source Code, AGPL License, Build Details, and Locale Data links without making raw registry status legends the default end-user experience.
* World Keystone may show a creator support link in Settings. Other locale deployments must not show the World Keystone Patreon link.
* Signed official releases may be distributed for convenience, but users must still be able to obtain the corresponding source required by the software license.
* Modified deployments must not claim official status unless they are authorized and verifiably running an approved release/configuration.
* Community deployments must preserve attribution and license notices.
* Official branding is reserved for Global Keystone and authorized locale instances.
* A locale instance may use the `{Locale Display Name} Keystone` pattern only in a way consistent with the trademark/brand policy, operator agreement, and registry status.
* The operator agreement template, release signing target, and registry status contract are tracked in `docs/operator-agreement-template.md`, `docs/release-signing-and-provenance.md`, and `docs/locale-registry-statuses.md`.

If the project later wants a separate proprietary operational appliance, managed hosting product, trademark license, certification mark, or dual-license offering, that should be treated as a separate legal/product decision and documented before launch.

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

A signed-in account with an unverified email must be gated to email verification only. The web UI must not expose board navigation, board content, submission controls, voting controls, discussion controls, account surfaces, or review-unlock copy until verification is complete.

Verification emails should prioritize a one-click verification link that opens the app, verifies the email with the expiring token, creates a normal session, and removes the token from the visible URL after use. A copy-paste verification code may remain available as a fallback for mail clients or browsers that do not open the link cleanly.

After email verification succeeds, the user should be routed into the required-review pool when required reviews are waiting. If no required reviews are waiting, the user may proceed to the normal verified app flow.

A newly created account must still satisfy the cycle review unlock rules before submitting or voting in a cycle.

### 5.6 First-time onboarding

First-time onboarding applies only after email verification. The verification gate takes priority over welcome/tutorial screens. After verification, the system should prefer the required-review handoff when required reviews are waiting; onboarding may appear before normal board use when it does not obscure the verification and required-review gates. The system may use `last_login_at` to determine first login. Local browser dismissal state must not suppress first-time onboarding when the account is reset for development testing.

After a user completes the required-review pool for the first time, the interface should show a one-time, non-skippable handoff tutorial. First, it should fully blur the board and say: "These are real submissions, by real people." When the user clicks the fade, it should show: "Voting is unlimited, so please vote on as many submissions as you can." When the user clicks again, it should transition to highlighting the first available submission in the board list and say: "Please click this one and open it." The tutorial should let the user click that highlighted submission. After the detail pane opens, the UI should blur again and say: "Scroll down to vote, discuss or flag this submission." Clicking the fade or scrolling down then closes the tutorial and returns full control; a scroll-down gesture should also move the detail pane downward. Local browser dismissal state must not suppress this handoff tutorial when the account is reset for development testing and returns with first-login onboarding required.

---

## 6. Users and Roles

### 6.1 User roles in v1

Version 1 recognizes:

* **Guest** — may access login, registration, account recovery, health-check, source/license, build-provenance, and locale-registry metadata surfaces only. Guest browsing of proposal, result, implementation, archive, or merge-relationship content is disabled for now.
* **Registered User** — verified email, may browse app content, complete review unlocks, submit proposals, vote, and appeal if eligible
* **Moderator** — may act only within specified moderation powers and thresholds

For implementation tracking, moderators also perform steward recordkeeping duties in v1.

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

Because proposals are not editable after submission, the UI must show a preview of the issue or solution as it will appear and require explicit confirmation before creating the proposal.

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

### 8.4 Submission discussion

Discussion is attached to individual issue and solution submissions. Version 1 does not include separate board-wide forums, direct messages, public profiles, usernames, or public user identifiers.

Discussion rules:

* only authenticated, email-verified users who have completed the relevant board review unlock may post or vote on comments
* each user may post at most one comment per submission
* the database must enforce the one-comment-per-user-per-submission rule
* authors may comment on their own submissions
* an author's comment may be labeled only as **Author**
* comment responses and UI must not expose emails, raw user IDs, public user IDs, usernames, profiles, or other identity breadcrumbs
* each new comment automatically starts with a like from its author
* users may like or dislike comments
* comment like/dislike counts and ratios must not be visible to standard users
* comments are sorted by hidden like-to-dislike ratio, then hidden net preference, then hidden total comment-vote activity, then oldest first
* comment voting affects only comment ordering and must not affect proposal ranking, outcome resolution, or proposal vote counts
* comments do not amend the official proposal text
* discussion closes when the submission is no longer active
* archived comments remain visible as historical context, but archived submissions do not accept new comments or comment votes in v1

### 8.5 Input size limits

Version 1 uses bounded proposal inputs to keep submissions reviewable and prevent oversized payloads:

* Proposal titles are capped at 120 characters.
* Long proposal descriptions, action descriptions, notes, appeals, and moderation explanations are capped at 2,000 characters.
* Affected people or scope is capped at 500 characters.
* Solution problem-fit explanations are capped at 1,000 characters.
* External implementation links are capped at 2,048 characters.

### 8.6 User-facing detail presentation

Proposal details should be presented as a centered, readable detail pane with a consistent maximum content width across title, description sections, and participation controls. Routine metadata such as author IDs, board labels, and creation timestamps should not be shown to standard users.

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

* **Resource Status** (`not_started`, `in_progress`, `secured`, or `blocked`)
* **Current Acquired Amount**
* **External Tracking Link**
* **Status / Proof Note**
* **Timestamp of last resource update**

In v1, implementation tracking links are added or changed by moderator-stewards after a solution wins.

When the target amount and acquired amount are numeric, the interface should display per-resource acquisition progress and remaining amount. Progress is calculated per resource entry because different entries may use incompatible units.

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
* **Evidence Link**
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

Keystone does not host implementation evidence files in v1. Spreadsheets, documents, folders, reports, dashboards, and payment/funding records should live in external tools. Keystone stores the external link, the short proof note, the resource or completion criterion it supports, the update timestamp, and moderator-steward interpretation of what the evidence proves.

### 9.7 Moderator-steward procedure in v1

Version 1 does not include separate paid staff, project managers, or implementation steward accounts.

Moderators therefore also act as implementation stewards for v1. This means moderators may maintain the official implementation record after a solution wins, including:

* attaching external tracking links
* updating acquired resource amounts
* updating resource and completion statuses
* adding evidence and proof notes
* recording a steward update note for the audit trail

External links should be view-only or public wherever possible. If a linked document, folder, spreadsheet, fundraiser, dashboard, or signup tool is not publicly viewable, the moderator-steward note should explain what it is and what has been verified.

Moderator-steward authority is custody of the implementation record, not unilateral final authority over success or failure.

### 9.8 Implementation finality

Moderator-stewards may update implementation progress, resource acquisition values, evidence links, proof notes, and ordinary status values such as `active` or `paused`.

Moderator-stewards may not directly mark an implementation `completed` or `cancelled` in v1. Completion and cancellation require a future claim/review flow or other community-ratified mechanism. Until that mechanism exists, Keystone should track progress without allowing a single moderator to finalize or terminate implementation status.

### 9.9 Locale boundaries for implementations

Implementation records belong to the locale instance and cycle that produced the winning solution.

A Castle Rock deployment, Douglas County deployment, World deployment, or other localized instance should track its own implementation records independently. Implementation status, resource progress, steward notes, and evidence links must not automatically cross from one locale deployment into another.

If a future central discovery or federation layer lists external Keystone deployments, it should identify the source deployment, locale, operator, provenance status, and last verified build. External implementation records may be referenced or linked, but they should not be silently blended into the canonical instance's own implementation records.

Users should discover and access those locale implementation records through the global locale access point. If the global site summarizes implementation progress from a locale deployment, the summary must preserve the source locale, operator, provenance status, and last verified update time.

This preserves local accountability: each locale's Keystone is responsible for the real-world follow-through selected by that locale's users.

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

The Issue Board may display the **Not a Fit** sentiment as **Downvote** in the user interface while preserving the internal `not_a_fit` vote value and count.

Primary sentiment controls should make the main decision visually clear without dominating the page: Support and Not a Fit / Downvote should share a consistent full-width control row, using subdued green and red tint treatments respectively. Secondary flag choices should remain visually quieter.

These are mutually exclusive.

### 11.2 Merge vote

Each user may also independently cast exactly one merge vote per active proposal:

* **Merge**

Each merge vote must identify the other active proposal it is targeting. Untargeted merge votes are not valid in v1 because merge thresholds are pair-specific.

Merge signaling is available on active proposals after the participant has completed the required review unlock for that board. It is part of the same active monthly participation window as sentiment voting.

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

After cycle closeout, the selected winner and its relevant outcome record must be published in an auditable form. Hidden live vote surfaces are intended to prevent mid-cycle score chasing, not to hide final outcome history after voting has ended.

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

The platform may publish final cycle results, including vote totals and outcome data, after the monthly cycle has closed.

### 12.4 User identifier visibility

Normal API and UI surfaces must not expose raw user UUIDs for proposal authors, moderators, appeal reviewers, reconsideration reviewers, or the currently logged-in account unless a future audited export specifically requires them. Moderator-facing queues should identify work by proposal, threshold signal, and action history rather than by real user identity.

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

Required-review progress shown to the user should be one-based while the user is actively reviewing: the first required review is shown as **Review 1/4**, not **0/4**. If fewer than four eligible reviewable submissions exist, the UI should also indicate the scaled-down available count and how many reviews remain after the current one.

Required review is an internal forced state, not a persistent user navigation section. After the unlock is complete, the user should not be able to reopen Required Reviews as a normal board section.

The normal Issue and Solution board feeds should reuse the required-review priority buckets as their default ordering, repeating the four-slot priority pattern across the full list rather than using a separate feed tab.

### 13.4.1 Review action contents

A required review action must require the participant to cast one of the normal sentiment choices on the reviewed proposal:

* Support
* Not a Fit
* Unclear
* Unsafe / Illegal / Deceptive

The Issue Board may display Not a Fit as Downvote in required-review and voting controls while preserving the internal `not_a_fit` vote value.

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

* unsafe_count / total_count >= 50%
* unsafe_count >= 8

### 19.3 Moderator action threshold rule

Moderators may not archive, freeze, or otherwise moderate active proposal content until High Moderation-Watch has been reached.

High Moderation-Watch must remain continuously active for **24 hours** before any harmful moderation action is available. If the proposal drops below High Moderation-Watch before the hold completes, the hold resets.

Merge actions are governed by the separate High Merge-Watch threshold and require an explicit merge relationship between the proposals.

Before that, moderators may observe flags and queue data only.

### 19.4 Consequences of High Moderation-Watch

When High Moderation-Watch has remained active for the required 24-hour hold, moderators may:

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

Canonical stored archive reason codes:

* `duplicate`
* `unsafe_illegal_deceptive`
* `spam_abuse`
* `irrelevant`
* `minimum_quality`
* `superseded`
* `moderation`
* `manual_archive`
* `not_a_fit`
* `merged`
* `cycle_closed`

The moderator archive endpoint accepts only active moderation reasons. `merged` and `cycle_closed` are reserved system lifecycle reasons and must not be selectable as ordinary moderator archive reasons.

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

Cycle result status values in v1 are:

* `resolved` - a ranked winner exists and was published
* `no_ranked_winner` - eligible candidates existed or the board was open, but no candidate met ranked-winner requirements
* `no_solution_target` - the Solution Board had no prior winning issue to solve

### 26.3 Winning solution transition

The winning solution must become an implementation tracking record without requiring re-entry of structured implementation data.

This is possible because the required execution fields already exist at submission time.

The normal product workflow must not expose manual implementation promotion from an ordinary solution detail view. Implementations are created by the same cycle-close mechanic that moves a winning issue into the next Solution Board target: the prior cycle's winning solution becomes the active implementation record. Any direct moderator-steward creation path is a recovery tool only and must still require a published winning solution result.

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
* BuildProvenanceManifest
* LocaleRegistryEntry
* DeploymentAttestation
* BootstrapModeratorToken

Exact schema design may normalize or combine some of these, but the concept boundaries must remain supported.

---

## 29. Non-Functional Requirements

Version 1 should prioritize:

* auditability
* deterministic rule application
* public build provenance for the canonical deployment
* easy relocalization from the public repository
* simple first-moderator bootstrap for new locale deployments
* clear separation of AGPL software rights from official brand rights
* low operational cost
* low moderation ambiguity where possible
* hidden live-score surfaces
* ease of later expansion into more locales and governance boards

Version 1 should avoid:

* unnecessary moderation discretion before threshold triggers
* hidden destructive actions
* user confusion about whether a proposal changed after voting
* attempting to build all external coordination tools natively
