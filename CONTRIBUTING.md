# Contributing to Collaborative Keystone

Thank you for contributing.

This project is in an early specification-first phase. Before large code contributions, the most valuable help is clarity: rules, edge cases, product structure, safety logic, governance logic, and implementation simplicity.

---

## Attribution

Please preserve attribution to the original project and its origin when contributing, discussing, or forking this repository.

Open source is not anonymity.
The goal is for the project to outlive its founder without erasing the fact that it began here.

---

## 1. Before You Contribute

Read these files first:

- `README.md`
- `ROADMAP.md`
- `GOVERNANCE.md`

As the project grows, also read:

- `docs/foundation.md`
- `docs/rules.md`
- `docs/srs-v1.md`

Do not assume the project is “just another forum.”
The mechanism matters more than the surface UI.

---

## 2. What We Need Most Right Now

The highest-value contributions in the current phase are:

- rule clarification
- edge-case analysis
- moderation logic refinement
- merge / archive / appeal logic review
- execution tracking design
- schema design
- wireframes
- boring, maintainable implementation ideas
- documentation improvements

Version 1 should be simple, strong, and reproducible.

---

## 3. Contribution Principles

Please optimize for:

- clarity
- simplicity
- transparency
- auditability
- maintainability
- forkability

Please avoid:

- overengineering
- premature complexity
- feature bloat
- hidden magic
- governance shortcuts
- “we can fix it later” logic in core rules

---

## 4. Good Contribution Types

Examples of helpful contributions:

- improving project docs
- identifying contradictions in the rules
- proposing better wording for proposal requirements
- tightening moderation edge cases
- suggesting safer data models
- improving onboarding copy
- designing clean wireframes
- implementing well-scoped product features
- writing tests for core logic

---

## 5. Poor Contribution Types

Examples of unhelpful contributions:

- giant rewrites without explanation
- undocumented architecture changes
- features that expand scope before the core mechanism works
- changes that weaken auditability
- changes that expose live vote data during active cycles
- proposals that silently shift governance powers
- changes that make self-hosting harder without strong reason

---

## 6. How to Propose Changes

For now, contribute using a simple process:

1. open an issue or discussion describing the problem
2. explain the proposed change in plain English
3. explain why it improves the mechanism
4. keep scope narrow where possible
5. submit code only after the intended behavior is clear

A good contribution explains both **what** changes and **why**.

---

## 7. Branching

Until the project has a larger team, keep branching simple.

Suggested pattern:

- `main` for stable work
- short-lived feature branches for active work

Example branch names:

- `docs/readme-foundation`
- `feature/proposal-review-gate`
- `feature/archive-board`
- `fix/merge-review-state`

---

## 8. Commit Messages

Keep commit messages direct and specific.

Examples:

- `Add initial governance document`
- `Define archive and appeal rules`
- `Create proposal state model`
- `Add execution record schema draft`

Bad examples:

- `stuff`
- `update`
- `fix things`
- `more changes`

---

## 9. Pull Request Expectations

A good pull request should:

- stay focused
- explain the reason for the change
- describe important behavior changes
- mention any rule or governance implications
- avoid unrelated cleanup mixed into the same PR

If a change affects governance, moderation, voting logic, archive logic, or execution tracking, call that out explicitly.

---

## 10. Documentation First for Core Logic

If you are changing a core mechanism, documentation should move with the code.

Core mechanism examples:

- voting flow
- submission flow
- merge review flow
- archive / unarchive rules
- appeal handling
- execution tracking model
- trust / verification model

If behavior changes but docs do not, the contribution is incomplete.

---

## 11. Style Expectations

Prioritize:

- understandable code
- minimal moving parts
- explicit state transitions
- configuration over hardcoded policy where practical
- logs and auditability where the mechanism depends on trust

This project should be easier to inspect than to mythologize.

---

## 12. Code of Conduct

Be serious, direct, and constructive.

Disagreement is normal.
Confusion is fixable.
Hidden agendas, sabotage, and bad-faith manipulation are not acceptable.

A separate `CODE_OF_CONDUCT.md` may expand this later.

---

## 13. Licensing

By contributing, you agree that your contributions are provided under the repository’s license.

---

## 14. Current Priority Order

1. foundation docs
2. rules/spec
3. schema design
4. wireframes
5. minimal implementation
6. anti-abuse hardening
7. execution tracking expansion
8. governance expansion