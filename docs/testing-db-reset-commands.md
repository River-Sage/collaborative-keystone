# Local Testing Database Reset Commands

These commands are for the local development database only. They assume this checkout path:

```powershell
C:\Dev\Sites\collaborative-keystone\collaborative-keystone
```

The helper reads `DATABASE_URL` from `site\api\.env` unless `$env:DATABASE_URL` is already set. It uses your local `psql.exe`, then calls the existing Rust demo seeder where appropriate.

## GUI Test Suite

For a button-driven workflow, run the Python GUI from VS Code or PowerShell:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone
python .\scripts\dev_db_test_gui.py
```

The GUI wraps the same PowerShell helper functions documented below, so the SQL and seeding logic still live in one place. It includes quick resets, realistic staging, per-user vote/login resets, verification/password reset scenarios, cycle phase switching, moderation scenarios, trust-review scenarios, duplicate-link reset, execution reset, and a live console.

To verify the GUI bridge without opening the window:

```powershell
python .\scripts\dev_db_test_gui.py --smoke
```

To run the baseline requirements sanity checks without opening the window:

```powershell
python .\scripts\dev_db_test_gui.py --sanity
```

Or from a loaded PowerShell helper session:

```powershell
Test-CkSeedRequirements
```

The baseline sanity check is read-only. It is intended to pass after `Reset-CkDatabaseFull`. It may intentionally fail after you create appeal, reconsideration, verification, password-reset, execution, or other niche scenarios because those commands move the database away from the clean baseline.

## Load The Helpers

Paste this once at the start of a PowerShell session:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev-db-test-tools.ps1
```

## Big Reset

This clears operational/test data, restores the three dev accounts, recreates active demo cycles/proposals/votes/merge relationships, seeds merge notification records for the demo duplicate-link pairs, and prints a summary.

```powershell
Reset-CkDatabaseFull
```

Dev accounts after reset:

```text
user@example.com       SuperSecurePass123
test2@example.com      SuperSecurePass123
moderator@example.com  SuperSecurePass123
```

## Realistic Staging

This starts from a clean reset, removes `DEMO` prefixes from visible submission titles, adds additional realistic issue and solution records, and reseeds vote counts so the normal Issues/Solutions feeds have enough material to exercise priority ordering.

```powershell
Stage-CkRealisticEnvironment
```

This is for browsing and product testing, not baseline seed sanity. Run `Reset-CkDatabaseFull` before `Test-CkSeedRequirements`.

## Quick Status

```powershell
Show-CkSeedSummary
```

## Reset Accounts Only

This restores passwords, roles, email verification, clears sessions/tokens, and resets `last_login_at` for the three dev accounts.

```powershell
Reset-CkDevAccounts
```

## Reset A User's Votes And Reviews

Useful when `test2@example.com` has already unlocked reviews, voted, or merge-signaled and you want to test those flows again.

```powershell
Reset-CkUserParticipation -Email test2@example.com
```

Other examples:

```powershell
Reset-CkUserParticipation -Email user@example.com
Reset-CkUserParticipation -Email moderator@example.com
```

## Reset A User's Login State

Clears active sessions, auth tokens, and `last_login_at`, while keeping the account verified.

```powershell
Reset-CkUserLoginState -Email test2@example.com
```

Note: API auth rate limits are in memory. If you are testing repeated failed logins and hit a rate limit, restart the API process too.

## Email Verification Scenario

Makes the user unverified and inserts a known verification token. Log in as the user, then use this token in the verification flow:

```powershell
New-CkVerificationScenario -Email test2@example.com -Token dev-verify-test2
```

Plain token:

```text
dev-verify-test2
```

## Password Reset Scenario

Inserts a known password reset token for the user.

```powershell
New-CkPasswordResetScenario -Email test2@example.com -Token dev-reset-test2
```

Plain token:

```text
dev-reset-test2
```

## Cycle Phase Switching

Submission phase:

```powershell
Set-CkCyclePhase -Phase submission
```

Voting phase:

```powershell
Set-CkCyclePhase -Phase voting
```

Closed phase:

```powershell
Set-CkCyclePhase -Phase closed
```

Use `closed` when testing moderator cycle outcome resolution. Use `submission` afterward to return to normal proposal-submission testing.

## Moderation Reset

Clears appeals, reconsiderations, moderator action logs, frozen flags, and restores demo proposals to their normal active/archived state.

```powershell
Reset-CkModerationFacet
```

## Trust Review Reset

Clears anti-abuse review flags and user activity signals, then restores the seeded merge notification records that the demo duplicate-link pairs need for audit testing.

```powershell
Reset-CkTrustFacet
```

## Trust Review Scenario

Creates one open trust-review flag against `test2@example.com` so the moderator Trust Review tab can be tested.

```powershell
New-CkTrustReviewScenario
```

Expected UI path:

```text
Log in as moderator@example.com -> Trust Review -> Acknowledge or Dismiss
```

## Appeal Scenario

Creates or resets an archived proposal owned by `test2@example.com`, with a moderator archive action. Log in as `test2@example.com`, open the Archive tab, and submit an appeal for:

```text
SCENARIO APPEAL: Archived test2 proposal
```

Command:

```powershell
New-CkAppealScenario
```

## Reconsideration Scenario

Creates or resets an archived proposal that a moderator can send into a 72-hour reconsideration window.

```text
SCENARIO RECONSIDERATION: Archived public proposal
```

Command:

```powershell
New-CkReconsiderationScenario
```

## Merge Reset

Clears merge votes/relationships/distinction notes/reconciliation logs, restores demo proposals, then reruns the demo seeder to recreate the merge-heavy scenarios.

```powershell
Reset-CkMergeFacet
```

Good merge test records:

```text
DEMO ISSUE: Duplicate clean water access framing
DEMO ISSUE: Clean water access gap
DEMO SOLUTION: Mobile water testing training corps
DEMO SOLUTION: Regional water lab network
```

## Execution Tracking Reset

Clears execution records and solution cycle results, then makes demo solution proposals active again.

```powershell
Reset-CkExecutionFacet
```

Good execution test record:

```text
DEMO SOLUTION: Regional water lab network
```

## One-Paste Full Reset

If you want a single paste from a fresh PowerShell window:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev-db-test-tools.ps1
Reset-CkDatabaseFull
```
