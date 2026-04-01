# Collaborative Keystone

Collaborative Keystone is an open source platform for democratic issue identification, solution development, and execution tracking.

The goal is not to solve everything at once.
The goal is to create a durable public process for identifying urgent problems, developing practical solutions, and tracking whether those solutions are actually completed.

## Attribution and Branding

Collaborative Keystone was originated by River Madrid.

The Collaborative Keystone codebase is open source under the GNU AGPL v3.0.
You are free to use, study, modify, and redistribute the code under that license.

However, the name **Collaborative Keystone**, its logo, visual identity, and official project branding are not granted under the software license.
Forks and derivatives must not present themselves as the official Collaborative Keystone project unless separately authorized.

If you create a fork or derivative deployment, you must provide clear attribution to the original project and clearly distinguish your version from the official project.

## Mission

Focus human attention, organize human effort, and turn shared concern into coordinated action.

No one person can solve humanity’s greatest problems alone. But together, people can identify priorities, share knowledge, build consensus, and support meaningful work in the world.

## Version 1 Scope

Version 1 begins with a single locale:

- **World**

Version 1 includes two primary boards:

- **Issue Board**  
  Community members submit and vote on the most pressing issue affecting the locale.

- **Solution Board**  
  Community members submit and vote on actionable solutions to the winning issue from the previous cycle.

Version 1 is intentionally narrow. It is designed to prove the core mechanism first.

## Core Cycle

Collaborative Keystone runs in repeating monthly cycles.

Recommended starting cadence:

- **Days 1–21:** issue submission and discussion
- **Days 22–30:** issue voting
- During the same cycle, the Solution Board develops and votes on solutions for the prior cycle’s winning issue

This creates a repeating rhythm:

- identify the problem
- develop the response
- track the execution
- repeat

## What Makes a Strong Proposal

### Issues must be:

- tangible real-world problems
- specific enough to be understood clearly
- relevant to the current locale
- distinct from existing proposals
- serious enough to justify public attention

### Solutions must be:

- actionable
- linked to a specific issue
- practical enough to attempt
- written with clear completion criteria
- clear enough that it is obvious when they are complete

## Voting Philosophy

The platform is designed to resist simple popularity snowballing and bot-driven manipulation.

Version 1 uses:

- one account per verified participant
- structured review before submission and voting unlock
- independent merge signaling
- archive instead of delete
- auditable moderation
- no public live vote totals during active cycles

Users do **not** see live vote counts while a cycle is active. This reduces bandwagon behavior, orchestrated attacks, and shallow copy-voting.

## Moderation Philosophy

Ordinary submissions are not silently erased.

Instead, version 1 uses:

- archive
- unarchive
- merge review
- appeal review
- audit logging

Archived proposals remain viewable and historically auditable.

## Merge Logic

A proposal that draws significant merge signaling may enter merge review.

When that happens:

- related proposals may be surfaced side by side
- the author may submit a distinction note explaining why their proposal is materially different
- moderators review the relationship
- merge actions are logged for posterity

Version 1 does **not** auto-merge proposals.

## Winning Solutions

Winning solutions do not end as text alone.

They become tracked execution records with, at minimum:

- linked winning solution
- required resource categories
- completion criteria
- external implementation links
- progress notes / evidence notes

Version 1 may rely on third-party platforms for fundraising, volunteer coordination, or partner coordination, but Collaborative Keystone remains the place where progress is tracked.

## Open by Design

This project is intended to belong to the public, not to a single individual.

Its code, structure, and guiding principles are being developed for transparency, replication, and long-term continuity so that communities at every scale may one day run their own versions.

## Project Principles

- build the core mechanism before expanding scope
- keep version 1 boring, strong, and reproducible
- favor transparency over hidden power
- favor auditable moderation over silent deletion
- favor practical proposals over slogans
- favor structured completion over vague aspiration

## Planned Future Expansion

Later versions may add:

- Platform Improvement Board
- Governance / Rules Board
- fork guides for city/state/country/world instances
- richer execution tracking
- stronger trust tiers for participation
- public governance processes for changing site rules

## Current Status

Planning and specification phase.

## Repository Structure

docs/
  foundation.md
  rules.md
  srs-v1.md
README.md
ROADMAP.md
GOVERNANCE.md
CONTRIBUTING.md
CODE_OF_CONDUCT.md
TRADEMARKS.md
LICENSE

## Contributing

See `CONTRIBUTING.md`

## Governance

See `GOVERNANCE.md`

## Roadmap

See `ROADMAP.md`

## License

GNU AGPL v3.0