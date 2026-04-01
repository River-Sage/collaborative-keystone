# Roadmap

This roadmap is intentionally conservative.

Collaborative Keystone should not try to build the entire future in version 1. It should prove the core mechanism first, document it well, and make the system reproducible.

---

## Phase 0 — Foundation

Goal: define the system clearly before writing production-grade code.

### Deliverables

- mission statement
- non-goals
- cycle rules
- issue submission rules
- solution submission rules
- moderation rules
- merge review rules
- archive / unarchive rules
- execution tracking minimum requirements
- public repository with foundational docs
- initial project structure

### Exit criteria

- version 1 logic is written in plain English
- repository is public
- core rules are stable enough to implement

---

## Phase 1 — Minimal Product

Goal: build a small but real version of Collaborative Keystone for the world locale.

### Core product

- World locale only
- monthly cycle
- Issue Board
- Solution Board
- account creation
- email verification
- proposal submission
- structured review requirement
- vote casting
- merge signaling
- archive board
- basic moderator tools
- cycle close and results release

### Required product rules

- no public live vote totals
- no public live ratio math
- no auto-merge
- no ordinary hard deletion
- no silent moderation
- no native fundraising engine
- no native volunteer marketplace

### Exit criteria

- users can submit issues
- users can review proposals
- users can unlock voting
- users can vote in a cycle
- moderators can archive / unarchive / merge / review appeals
- results can be published after cycle close

---

## Phase 2 — Execution Tracking

Goal: make winning solutions transition into tracked implementation records.

### Deliverables

- execution record creation from winning solution
- structured completion criteria
- resource category tracking
- external link support
- progress notes / evidence notes
- completion status display

### Resource categories

- money
- labor / manpower
- skills / trades
- materials
- equipment
- logistics / transport
- organizational support
- other

### Exit criteria

- each winning solution can become a tracked implementation object
- users can see what is required, what is complete, and what remains

---

## Phase 3 — Trust and Anti-Abuse Hardening

Goal: strengthen legitimacy without killing participation.

### Possible additions

- account seasoning requirements
- phone verification for higher-trust actions
- anomaly detection
- device / IP heuristics
- moderator review queue improvements
- trusted-tier participation for governance boards

### Exit criteria

- fraud resistance is meaningfully stronger than v1
- suspicious behavior is easier to detect and review

---

## Phase 4 — Community Governance Expansion

Goal: let the platform evolve without becoming chaotic or self-destructive.

### Planned future boards

- Platform Improvement Board
- Governance / Rules Board

### Principles

- changes to the platform itself should have higher friction than ordinary issue / solution proposals
- governance changes should be auditable
- critical system rules should not be instantly mutable by low-trust participation

### Exit criteria

- platform changes have a safe and reviewable governance path
- community input can shape the platform without destabilizing it

---

## Phase 5 — Forkability and Replication

Goal: let communities run their own versions.

### Deliverables

- self-hosting guide
- setup guide
- sample environment files
- fork guide for new locales
- branding separation from engine
- locale configuration model

### Exit criteria

- a new maintainer can spin up a local instance without depending on the original founder
- the mechanism survives beyond one repo, one domain, or one person

---

## Non-Goals for Version 1

Version 1 is **not** trying to be:

- a full social network
- a native crowdfunding platform
- a native volunteer staffing marketplace
- a perfect one-human-one-account identity system
- an everything-app for public policy
- a final form of governance for the platform

Version 1 exists to prove the mechanism.

---

## Immediate Next Work

1. Foundation docs
2. product rules spec
3. data model
4. moderation model
5. wireframes
6. implementation plan
7. first build