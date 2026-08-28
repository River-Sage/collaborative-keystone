use chrono::{Duration, Utc};
use dotenvy::dotenv;
use serde_json::{Value, json};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::path::PathBuf;
use uuid::Uuid;

const SEED_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$ZGVtby1zZWVkLXNvdXJjZQ$1P+4nXhC2qVz6S2nUijQwzGxg8k0Uo4QyPvH8Dg6S1A";

#[derive(Clone)]
struct SeedProposal {
    title: &'static str,
    board_code: &'static str,
    problem_description: Option<&'static str>,
    affected_scope: Option<&'static str>,
    why_it_matters: Option<&'static str>,
    action_description: Option<&'static str>,
    required_resource_categories: Option<Value>,
    completion_criteria: Option<Value>,
    execution_tracking_entries: Option<Value>,
    votes: VotePlan,
}

#[derive(Clone, Copy)]
struct VotePlan {
    support: usize,
    not_a_fit: usize,
    unclear: usize,
    unsafe_count: usize,
}

struct SeededProposal {
    id: Uuid,
    title: &'static str,
    votes: VotePlan,
}

struct ActiveCycle {
    id: Uuid,
    locale_id: Uuid,
    cycle_number: i32,
}

struct SeedLocale {
    slug: String,
    name: String,
}

impl SeedLocale {
    fn from_env() -> Self {
        let slug = std::env::var("CK_LOCALE_SLUG")
            .ok()
            .map(|value| normalize_slug(&value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "world".to_string());
        let name = std::env::var("CK_LOCALE_NAME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if slug == "world" {
                    "World".to_string()
                } else {
                    slug.split('-')
                        .filter(|part| !part.is_empty())
                        .map(|part| {
                            let mut chars = part.chars();
                            match chars.next() {
                                Some(first) => {
                                    first.to_ascii_uppercase().to_string()
                                        + chars.as_str().to_ascii_lowercase().as_str()
                                }
                                None => String::new(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            });

        Self { slug, name }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let locale = SeedLocale::from_env();
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in site/api/.env");
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let migrations = sqlx::migrate::Migrator::new(migrations_path().as_path()).await?;
    migrations.run(&db).await?;

    let active_cycle = ensure_active_cycle(&db, &locale).await?;
    let issue_board_id = board_id(&db, "issue").await?;
    let solution_board_id = board_id(&db, "solution").await?;

    let issue_author_id = ensure_seed_user(&db, "seed-author-issue@example.test").await?;
    let solution_author_id = ensure_seed_user(&db, "seed-author-solution@example.test").await?;
    let relationship_creator_id = ensure_seed_user(&db, "seed-relationship@example.test").await?;
    let voter_ids = ensure_seed_voters(&db, 36).await?;

    let solution_target_issue_id = ensure_solution_target_issue(
        &db,
        &active_cycle,
        issue_board_id,
        issue_author_id,
        &voter_ids,
    )
    .await?;

    let issue_seeds = vec![
        SeedProposal {
            title: "DEMO ISSUE: Clean water access gap",
            board_code: "issue",
            problem_description: Some(
                "Many communities still lack reliable access to safe drinking water and clear local reporting.",
            ),
            affected_scope: Some("Rural and low-income communities across the current locale."),
            why_it_matters: Some(
                "Water access affects health, education, local economies, and emergency resilience.",
            ),
            action_description: None,
            required_resource_categories: None,
            completion_criteria: None,
            execution_tracking_entries: None,
            votes: VotePlan {
                support: 24,
                not_a_fit: 1,
                unclear: 1,
                unsafe_count: 0,
            },
        },
        SeedProposal {
            title: "DEMO ISSUE: AI transition safety net",
            board_code: "issue",
            problem_description: Some(
                "Automation and AI tools may rapidly displace workers before training and support systems catch up.",
            ),
            affected_scope: Some(
                "Workers in logistics, administration, media, support, and entry-level knowledge roles.",
            ),
            why_it_matters: Some(
                "A messy transition could produce preventable instability even if the technology is useful long term.",
            ),
            action_description: None,
            required_resource_categories: None,
            completion_criteria: None,
            execution_tracking_entries: None,
            votes: VotePlan {
                support: 10,
                not_a_fit: 8,
                unclear: 2,
                unsafe_count: 0,
            },
        },
        SeedProposal {
            title: "DEMO ISSUE: Antimicrobial resistance surge",
            board_code: "issue",
            problem_description: Some(
                "Drug-resistant infections are increasing while public awareness and stewardship remain uneven.",
            ),
            affected_scope: Some(
                "Hospitals, farms, clinics, and communities with limited access to diagnostics.",
            ),
            why_it_matters: Some(
                "Resistance makes routine infections and surgeries more dangerous and expensive.",
            ),
            action_description: None,
            required_resource_categories: None,
            completion_criteria: None,
            execution_tracking_entries: None,
            votes: VotePlan {
                support: 16,
                not_a_fit: 2,
                unclear: 2,
                unsafe_count: 1,
            },
        },
        SeedProposal {
            title: "DEMO ISSUE: Duplicate clean water access framing",
            board_code: "issue",
            problem_description: Some(
                "A closely related framing of the clean water access problem with emphasis on testing and reporting.",
            ),
            affected_scope: Some("Communities lacking reliable public testing data."),
            why_it_matters: Some(
                "This should exercise merge signaling against the broader clean water proposal.",
            ),
            action_description: None,
            required_resource_categories: None,
            completion_criteria: None,
            execution_tracking_entries: None,
            votes: VotePlan {
                support: 8,
                not_a_fit: 0,
                unclear: 0,
                unsafe_count: 0,
            },
        },
    ];

    let solution_seeds = vec![
        SeedProposal {
            title: "DEMO SOLUTION: Regional water lab network",
            board_code: "solution",
            problem_description: None,
            affected_scope: None,
            why_it_matters: Some(
                "Shared labs reduce the gap between suspected contamination and verified public results, giving underserved communities faster evidence for boil notices, repairs, and emergency support.",
            ),
            action_description: Some(
                "Create a regional network of water testing labs with shared reporting templates and public result dashboards.",
            ),
            required_resource_categories: Some(json!([
                "money",
                "equipment",
                "labor",
                "organizational support"
            ])),
            completion_criteria: Some(json!([
                {
                    "criterion_description": "Publish a public lab directory and reporting template.",
                    "completion_status": "not_started",
                    "evidence_note": "",
                    "updated_at": null
                },
                {
                    "criterion_description": "Run three pilot testing cycles with published findings.",
                    "completion_status": "not_started",
                    "evidence_note": "",
                    "updated_at": null
                }
            ])),
            execution_tracking_entries: Some(json!([
                {
                    "resource_category": "equipment",
                    "target_needed": "Water testing kits and sample transport supplies for pilot regions.",
                    "current_acquired_amount": "",
                    "external_coordination_link": "",
                    "status_proof_note": "Pilot resources not yet acquired."
                }
            ])),
            votes: VotePlan {
                support: 22,
                not_a_fit: 1,
                unclear: 0,
                unsafe_count: 0,
            },
        },
        SeedProposal {
            title: "DEMO SOLUTION: Water safety training toolkit",
            board_code: "solution",
            problem_description: None,
            affected_scope: None,
            why_it_matters: Some(
                "Training local testers and translators increases safe sampling capacity and helps households understand water results before small problems become chronic exposure risks.",
            ),
            action_description: Some(
                "Publish open training materials for safe water testing, sample handling, result reporting, and household response steps.",
            ),
            required_resource_categories: Some(json!([
                "skills / trades",
                "organizational support",
                "labor"
            ])),
            completion_criteria: Some(json!([
                {
                    "criterion_description": "Release water safety training materials under an open license.",
                    "completion_status": "not_started",
                    "evidence_note": "",
                    "updated_at": null
                }
            ])),
            execution_tracking_entries: Some(json!([
                {
                    "resource_category": "skills / trades",
                    "target_needed": "Water quality experts, technical writers, and translation reviewers.",
                    "current_acquired_amount": "",
                    "external_coordination_link": "",
                    "status_proof_note": "Needs expert review and field testing."
                }
            ])),
            votes: VotePlan {
                support: 14,
                not_a_fit: 3,
                unclear: 5,
                unsafe_count: 0,
            },
        },
        SeedProposal {
            title: "DEMO SOLUTION: Contamination alert verification hub",
            board_code: "solution",
            problem_description: None,
            affected_scope: None,
            why_it_matters: Some(
                "A verification hub helps communities distinguish real contamination alerts from rumors and directs people to safe pickup points when drinking water reliability breaks down.",
            ),
            action_description: Some(
                "Coordinate trusted local sources to verify water contamination alerts, boil notices, and safe pickup points.",
            ),
            required_resource_categories: Some(json!([
                "labor",
                "logistics / transport",
                "organizational support"
            ])),
            completion_criteria: Some(json!([
                {
                    "criterion_description": "Operate a simulated contamination-alert drill with documented response times.",
                    "completion_status": "not_started",
                    "evidence_note": "",
                    "updated_at": null
                }
            ])),
            execution_tracking_entries: Some(json!([
                {
                    "resource_category": "labor",
                    "target_needed": "Volunteer verification shifts and trusted-source contact lists for a 72-hour drill.",
                    "current_acquired_amount": "",
                    "external_coordination_link": "",
                    "status_proof_note": "Verification partner list pending."
                }
            ])),
            votes: VotePlan {
                support: 7,
                not_a_fit: 7,
                unclear: 4,
                unsafe_count: 1,
            },
        },
        SeedProposal {
            title: "DEMO SOLUTION: Mobile water testing training corps",
            board_code: "solution",
            problem_description: None,
            affected_scope: None,
            why_it_matters: Some(
                "Mobile testing teams bring water-quality checks to places without nearby labs, which makes unsafe drinking water easier to detect and escalate quickly.",
            ),
            action_description: Some(
                "Train mobile teams to test water quality, document results, and escalate urgent contamination findings.",
            ),
            required_resource_categories: Some(json!(["labor", "equipment", "skills / trades"])),
            completion_criteria: Some(json!([
                {
                    "criterion_description": "Train and document the first five mobile testing teams.",
                    "completion_status": "not_started",
                    "evidence_note": "",
                    "updated_at": null
                }
            ])),
            execution_tracking_entries: Some(json!([
                {
                    "resource_category": "labor",
                    "target_needed": "Five trained mobile teams.",
                    "current_acquired_amount": "",
                    "external_coordination_link": "",
                    "status_proof_note": "Training curriculum not started."
                }
            ])),
            votes: VotePlan {
                support: 10,
                not_a_fit: 1,
                unclear: 0,
                unsafe_count: 0,
            },
        },
    ];

    let mut seeded = Vec::new();
    for proposal in issue_seeds {
        seeded.push(
            upsert_seed_proposal(
                &db,
                &active_cycle,
                issue_board_id,
                issue_author_id,
                None,
                proposal,
            )
            .await?,
        );
    }

    for proposal in solution_seeds {
        seeded.push(
            upsert_seed_proposal(
                &db,
                &active_cycle,
                solution_board_id,
                solution_author_id,
                Some(solution_target_issue_id),
                proposal,
            )
            .await?,
        );
    }

    let clean_water_issue = seeded
        .iter()
        .find(|proposal| proposal.title == "DEMO ISSUE: Clean water access gap")
        .expect("clean water issue seeded")
        .id;
    let duplicate_water_issue = seeded
        .iter()
        .find(|proposal| proposal.title == "DEMO ISSUE: Duplicate clean water access framing")
        .expect("duplicate water issue seeded")
        .id;
    let water_lab_solution = seeded
        .iter()
        .find(|proposal| proposal.title == "DEMO SOLUTION: Regional water lab network")
        .expect("water lab solution seeded")
        .id;
    let mobile_water_solution = seeded
        .iter()
        .find(|proposal| proposal.title == "DEMO SOLUTION: Mobile water testing training corps")
        .expect("mobile water solution seeded")
        .id;

    for proposal in &seeded {
        clear_seed_votes(&db, proposal.id).await?;
    }

    for proposal in &seeded {
        seed_sentiment_votes(&db, proposal.id, proposal.votes, &voter_ids).await?;
    }

    seed_merge_relationship(
        &db,
        duplicate_water_issue,
        clean_water_issue,
        relationship_creator_id,
    )
    .await?;
    seed_merge_votes(
        &db,
        duplicate_water_issue,
        clean_water_issue,
        13,
        &voter_ids,
    )
    .await?;

    seed_merge_relationship(
        &db,
        mobile_water_solution,
        water_lab_solution,
        relationship_creator_id,
    )
    .await?;
    seed_merge_votes(
        &db,
        mobile_water_solution,
        water_lab_solution,
        11,
        &voter_ids,
    )
    .await?;

    for proposal in &seeded {
        refresh_vote_counts(&db, proposal.id).await?;
    }

    seed_merge_watch_notifications(&db).await?;

    println!("seeded_demo_submissions={}", seeded.len());
    println!("solution_target_issue_id={}", solution_target_issue_id);
    for proposal in seeded {
        let counts = load_counts(&db, proposal.id).await?;
        println!(
            "{} support={} not_fit={} unclear={} unsafe={} merge={} {}",
            proposal.id,
            counts.support,
            counts.not_a_fit,
            counts.unclear,
            counts.unsafe_count,
            counts.merge,
            proposal.title
        );
    }

    Ok(())
}

fn normalize_slug(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_dash = false;

    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch == '-' || ch == '_' || ch.is_whitespace() {
            Some('-')
        } else {
            None
        };

        if let Some(next) = next {
            if next == '-' {
                if output.is_empty() || previous_was_dash {
                    continue;
                }
                previous_was_dash = true;
            } else {
                previous_was_dash = false;
            }
            output.push(next);
        }
    }

    output.trim_matches('-').to_string()
}

fn migrations_path() -> PathBuf {
    std::env::var("CK_MIGRATIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../db/migrations"))
}

struct Counts {
    support: i32,
    not_a_fit: i32,
    unclear: i32,
    unsafe_count: i32,
    merge: i32,
}

async fn load_counts(db: &PgPool, proposal_id: Uuid) -> Result<Counts, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT support_count, not_a_fit_count, unclear_count, unsafe_count, merge_count
        FROM proposals
        WHERE id = $1
        "#,
    )
    .bind(proposal_id)
    .fetch_one(db)
    .await?;

    Ok(Counts {
        support: row.try_get("support_count")?,
        not_a_fit: row.try_get("not_a_fit_count")?,
        unclear: row.try_get("unclear_count")?,
        unsafe_count: row.try_get("unsafe_count")?,
        merge: row.try_get("merge_count")?,
    })
}

async fn ensure_active_cycle(db: &PgPool, locale: &SeedLocale) -> Result<ActiveCycle, sqlx::Error> {
    let locale_id: Uuid = sqlx::query(
        r#"
        INSERT INTO locales (slug, name, is_active)
        VALUES ($1, $2, TRUE)
        ON CONFLICT (slug)
        DO UPDATE SET
            name = EXCLUDED.name,
            is_active = TRUE
        RETURNING id
        "#,
    )
    .bind(&locale.slug)
    .bind(&locale.name)
    .fetch_one(db)
    .await?
    .try_get("id")?;

    let row = sqlx::query(
        r#"
        SELECT c.id, c.locale_id, c.cycle_number
        FROM cycles c
        WHERE c.locale_id = $1
          AND c.is_active = TRUE
        ORDER BY c.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(locale_id)
    .fetch_optional(db)
    .await?;

    if let Some(row) = row {
        return Ok(ActiveCycle {
            id: row.try_get("id")?,
            locale_id: row.try_get("locale_id")?,
            cycle_number: row.try_get("cycle_number")?,
        });
    }

    let starts_at = Utc::now();
    let row = sqlx::query(
        r#"
        INSERT INTO cycles (
            locale_id,
            cycle_number,
            starts_at,
            submission_ends_at,
            voting_ends_at,
            is_active
        )
        VALUES ($1, 1, $2, $3, $4, TRUE)
        RETURNING id, locale_id, cycle_number
        "#,
    )
    .bind(locale_id)
    .bind(starts_at)
    .bind(starts_at + Duration::days(30))
    .bind(starts_at + Duration::days(30))
    .fetch_one(db)
    .await?;

    Ok(ActiveCycle {
        id: row.try_get("id")?,
        locale_id: row.try_get("locale_id")?,
        cycle_number: row.try_get("cycle_number")?,
    })
}

async fn board_id(db: &PgPool, code: &str) -> Result<Uuid, sqlx::Error> {
    sqlx::query("SELECT id FROM boards WHERE code = $1 LIMIT 1")
        .bind(code)
        .fetch_one(db)
        .await?
        .try_get("id")
}

async fn ensure_seed_user(db: &PgPool, email: &str) -> Result<Uuid, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO users (email, password_hash, email_verified, role_code)
        VALUES ($1, $2, TRUE, 'registered_user')
        ON CONFLICT (email)
        DO UPDATE SET email_verified = TRUE, role_code = 'registered_user'
        RETURNING id
        "#,
    )
    .bind(email)
    .bind(SEED_PASSWORD_HASH)
    .fetch_one(db)
    .await?
    .try_get("id")
}

async fn ensure_seed_voters(db: &PgPool, count: usize) -> Result<Vec<Uuid>, sqlx::Error> {
    let mut users = Vec::with_capacity(count);
    for index in 1..=count {
        let email = format!("seed-voter-{index:02}@example.test");
        users.push(ensure_seed_user(db, &email).await?);
    }
    Ok(users)
}

async fn ensure_solution_target_issue(
    db: &PgPool,
    active_cycle: &ActiveCycle,
    issue_board_id: Uuid,
    author_id: Uuid,
    voter_ids: &[Uuid],
) -> Result<Uuid, sqlx::Error> {
    if let Some(id) = latest_prior_published_issue_winner(db, active_cycle).await? {
        return Ok(id);
    }

    let prior_cycle_number = active_cycle.cycle_number - 1;
    let starts_at = Utc::now() - Duration::days(35);
    let prior_cycle_id: Uuid = sqlx::query(
        r#"
        INSERT INTO cycles (
            locale_id,
            cycle_number,
            starts_at,
            submission_ends_at,
            voting_ends_at,
            is_active
        )
        VALUES ($1, $2, $3, $4, $5, FALSE)
        ON CONFLICT (locale_id, cycle_number)
        DO UPDATE SET is_active = FALSE
        RETURNING id
        "#,
    )
    .bind(active_cycle.locale_id)
    .bind(prior_cycle_number)
    .bind(starts_at)
    .bind(starts_at + Duration::days(30))
    .bind(starts_at + Duration::days(30))
    .fetch_one(db)
    .await?
    .try_get("id")?;

    let issue_id = upsert_proposal_row(
        db,
        prior_cycle_id,
        active_cycle.locale_id,
        issue_board_id,
        author_id,
        None,
        "DEMO PRIOR WINNER: Clean water as current solution target",
        Some("Prior-cycle winning issue used to make the current Solution Board active."),
        Some("Current locale"),
        Some("Seeded so solution proposals have a valid published target."),
        None,
        None,
        None,
        None,
        "archived",
        Some("cycle_closed"),
    )
    .await?;

    clear_seed_votes(db, issue_id).await?;
    seed_sentiment_votes(
        db,
        issue_id,
        VotePlan {
            support: 18,
            not_a_fit: 1,
            unclear: 1,
            unsafe_count: 0,
        },
        voter_ids,
    )
    .await?;
    refresh_vote_counts(db, issue_id).await?;

    sqlx::query(
        r#"
        INSERT INTO cycle_results (
            cycle_id,
            locale_id,
            board_code,
            winning_proposal_id,
            result_status,
            result_snapshot,
            published_at
        )
        VALUES (
            $1,
            $2,
            'issue',
            $3,
            'resolved',
            jsonb_build_object('seeded', true, 'title', 'DEMO PRIOR WINNER: Clean water as current solution target'),
            NOW()
        )
        ON CONFLICT (cycle_id, board_code)
        DO UPDATE SET
            winning_proposal_id = EXCLUDED.winning_proposal_id,
            result_status = 'resolved',
            result_snapshot = EXCLUDED.result_snapshot,
            published_at = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(prior_cycle_id)
    .bind(active_cycle.locale_id)
    .bind(issue_id)
    .execute(db)
    .await?;

    Ok(issue_id)
}

async fn latest_prior_published_issue_winner(
    db: &PgPool,
    active_cycle: &ActiveCycle,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT cr.winning_proposal_id
        FROM cycle_results cr
        JOIN cycles c ON c.id = cr.cycle_id
        WHERE cr.locale_id = $1
          AND c.cycle_number < $2
          AND cr.board_code = 'issue'
          AND cr.result_status = 'resolved'
          AND cr.winning_proposal_id IS NOT NULL
          AND cr.published_at IS NOT NULL
        ORDER BY c.cycle_number DESC, cr.published_at DESC
        LIMIT 1
        "#,
    )
    .bind(active_cycle.locale_id)
    .bind(active_cycle.cycle_number)
    .fetch_optional(db)
    .await?;

    row.map(|row| row.try_get("winning_proposal_id"))
        .transpose()
}

async fn upsert_seed_proposal(
    db: &PgPool,
    active_cycle: &ActiveCycle,
    board_id: Uuid,
    author_id: Uuid,
    parent_issue_id: Option<Uuid>,
    proposal: SeedProposal,
) -> Result<SeededProposal, sqlx::Error> {
    debug_assert!(proposal.board_code == "issue" || proposal.board_code == "solution");
    let id = upsert_proposal_row(
        db,
        active_cycle.id,
        active_cycle.locale_id,
        board_id,
        author_id,
        parent_issue_id,
        proposal.title,
        proposal.problem_description,
        proposal.affected_scope,
        proposal.why_it_matters,
        proposal.action_description,
        proposal.required_resource_categories,
        proposal.completion_criteria,
        proposal.execution_tracking_entries,
        "active",
        None,
    )
    .await?;

    Ok(SeededProposal {
        id,
        title: proposal.title,
        votes: proposal.votes,
    })
}

#[allow(clippy::too_many_arguments)]
async fn upsert_proposal_row(
    db: &PgPool,
    cycle_id: Uuid,
    locale_id: Uuid,
    board_id: Uuid,
    author_id: Uuid,
    parent_issue_id: Option<Uuid>,
    title: &str,
    problem_description: Option<&str>,
    affected_scope: Option<&str>,
    why_it_matters: Option<&str>,
    action_description: Option<&str>,
    required_resource_categories: Option<Value>,
    completion_criteria: Option<Value>,
    execution_tracking_entries: Option<Value>,
    primary_state: &str,
    archived_reason: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    if let Some(row) = sqlx::query("SELECT id FROM proposals WHERE title = $1 LIMIT 1")
        .bind(title)
        .fetch_optional(db)
        .await?
    {
        let id: Uuid = row.try_get("id")?;
        sqlx::query(
            r#"
            UPDATE proposals
            SET
                board_id = $2,
                cycle_id = $3,
                locale_id = $4,
                author_user_id = $5,
                parent_issue_proposal_id = $6,
                problem_description = $7,
                affected_scope = $8,
                why_it_matters = $9,
                action_description = $10,
                required_resource_categories = $11,
                completion_criteria = $12,
                execution_tracking_entries = $13,
                primary_state = $14,
                archived_reason = $15,
                moderation_note = NULL,
                merged_into_proposal_id = NULL
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(board_id)
        .bind(cycle_id)
        .bind(locale_id)
        .bind(author_id)
        .bind(parent_issue_id)
        .bind(problem_description)
        .bind(affected_scope)
        .bind(why_it_matters)
        .bind(action_description)
        .bind(required_resource_categories)
        .bind(completion_criteria)
        .bind(execution_tracking_entries)
        .bind(primary_state)
        .bind(archived_reason)
        .execute(db)
        .await?;
        return Ok(id);
    }

    sqlx::query(
        r#"
        INSERT INTO proposals (
            board_id,
            cycle_id,
            locale_id,
            author_user_id,
            parent_issue_proposal_id,
            title,
            problem_description,
            affected_scope,
            why_it_matters,
            action_description,
            required_resource_categories,
            completion_criteria,
            execution_tracking_entries,
            primary_state,
            archived_reason
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8, $9, $10,
            $11, $12, $13,
            $14, $15
        )
        RETURNING id
        "#,
    )
    .bind(board_id)
    .bind(cycle_id)
    .bind(locale_id)
    .bind(author_id)
    .bind(parent_issue_id)
    .bind(title)
    .bind(problem_description)
    .bind(affected_scope)
    .bind(why_it_matters)
    .bind(action_description)
    .bind(required_resource_categories)
    .bind(completion_criteria)
    .bind(execution_tracking_entries)
    .bind(primary_state)
    .bind(archived_reason)
    .fetch_one(db)
    .await?
    .try_get("id")
}

async fn clear_seed_votes(db: &PgPool, proposal_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM proposal_merge_votes
        WHERE proposal_id = $1
          AND user_id IN (
            SELECT id
            FROM users
            WHERE email LIKE 'seed-voter-%@example.test'
          )
        "#,
    )
    .bind(proposal_id)
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM proposal_sentiment_votes
        WHERE proposal_id = $1
          AND user_id IN (
            SELECT id
            FROM users
            WHERE email LIKE 'seed-voter-%@example.test'
          )
        "#,
    )
    .bind(proposal_id)
    .execute(db)
    .await?;

    Ok(())
}

async fn seed_sentiment_votes(
    db: &PgPool,
    proposal_id: Uuid,
    plan: VotePlan,
    voter_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    let mut offset = 0;
    insert_votes(
        db,
        proposal_id,
        "support",
        &voter_ids[offset..offset + plan.support],
    )
    .await?;
    offset += plan.support;
    insert_votes(
        db,
        proposal_id,
        "not_a_fit",
        &voter_ids[offset..offset + plan.not_a_fit],
    )
    .await?;
    offset += plan.not_a_fit;
    insert_votes(
        db,
        proposal_id,
        "unclear",
        &voter_ids[offset..offset + plan.unclear],
    )
    .await?;
    offset += plan.unclear;
    insert_votes(
        db,
        proposal_id,
        "unsafe",
        &voter_ids[offset..offset + plan.unsafe_count],
    )
    .await
}

async fn insert_votes(
    db: &PgPool,
    proposal_id: Uuid,
    vote_value: &str,
    voters: &[Uuid],
) -> Result<(), sqlx::Error> {
    for user_id in voters {
        sqlx::query(
            r#"
            INSERT INTO proposal_sentiment_votes (proposal_id, user_id, vote_value)
            VALUES ($1, $2, $3)
            ON CONFLICT (proposal_id, user_id)
            DO UPDATE SET vote_value = EXCLUDED.vote_value, updated_at = NOW()
            "#,
        )
        .bind(proposal_id)
        .bind(user_id)
        .bind(vote_value)
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn seed_merge_relationship(
    db: &PgPool,
    source_id: Uuid,
    target_id: Uuid,
    created_by_user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO proposal_merge_relationships (
            source_proposal_id,
            target_proposal_id,
            created_by_user_id,
            status
        )
        VALUES ($1, $2, $3, 'active')
        ON CONFLICT (source_proposal_id, target_proposal_id)
        DO UPDATE SET status = 'active', updated_at = NOW()
        "#,
    )
    .bind(source_id)
    .bind(target_id)
    .bind(created_by_user_id)
    .execute(db)
    .await?;
    Ok(())
}

async fn seed_merge_votes(
    db: &PgPool,
    source_id: Uuid,
    target_id: Uuid,
    count: usize,
    voter_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    for user_id in voter_ids.iter().take(count) {
        sqlx::query(
            r#"
            INSERT INTO proposal_merge_votes (proposal_id, user_id, target_proposal_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (proposal_id, user_id)
            DO UPDATE SET target_proposal_id = EXCLUDED.target_proposal_id, updated_at = NOW()
            "#,
        )
        .bind(source_id)
        .bind(user_id)
        .bind(target_id)
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn seed_merge_watch_notifications(db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        WITH merge_watch_relationships AS (
            SELECT
                r.source_proposal_id,
                r.target_proposal_id,
                source.author_user_id,
                source.title AS source_title,
                target.title AS target_title,
                (
                    source.support_count
                    + source.not_a_fit_count
                    + source.unclear_count
                    + source.unsafe_count
                    + source.merge_count
                ) AS source_total_count,
                source.merge_count AS source_merge_count
            FROM proposal_merge_relationships r
            JOIN proposals source ON source.id = r.source_proposal_id
            JOIN proposals target ON target.id = r.target_proposal_id
            WHERE r.status = 'active'
              AND source.primary_state = 'active'
              AND target.primary_state = 'active'
              AND source.title LIKE 'DEMO %'
              AND (
                    source.support_count
                    + source.not_a_fit_count
                    + source.unclear_count
                    + source.unsafe_count
                    + source.merge_count
                  ) >= 10
              AND source.merge_count::numeric
                  / NULLIF((
                        source.support_count
                        + source.not_a_fit_count
                        + source.unclear_count
                        + source.unsafe_count
                        + source.merge_count
                    ), 0)::numeric >= 0.20
        ),
        notification_recipients AS (
            SELECT
                author_user_id AS recipient_user_id,
                'merge_watch_author' AS notification_type,
                source_proposal_id,
                target_proposal_id,
                source_title,
                target_title,
                source_total_count,
                source_merge_count
            FROM merge_watch_relationships

            UNION ALL

            SELECT
                moderator.id AS recipient_user_id,
                'merge_watch_moderator' AS notification_type,
                relationship.source_proposal_id,
                relationship.target_proposal_id,
                relationship.source_title,
                relationship.target_title,
                relationship.source_total_count,
                relationship.source_merge_count
            FROM merge_watch_relationships relationship
            JOIN users moderator
              ON moderator.role_code = 'moderator'
             AND moderator.email_verified = TRUE
        )
        INSERT INTO notification_events (
            recipient_user_id,
            notification_type,
            proposal_id,
            related_proposal_id,
            payload
        )
        SELECT
            recipient_user_id,
            notification_type,
            source_proposal_id,
            target_proposal_id,
            jsonb_build_object(
                'summary', 'Duplicate signals have reached the author distinction-note threshold.',
                'source_title', source_title,
                'target_title', target_title,
                'source_total_count', source_total_count,
                'source_merge_count', source_merge_count,
                'seeded', TRUE
            )
        FROM notification_recipients
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn refresh_vote_counts(db: &PgPool, proposal_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        WITH counts AS (
            SELECT
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'support'
                ) AS support_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'not_a_fit'
                ) AS not_a_fit_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'unclear'
                ) AS unclear_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'unsafe'
                ) AS unsafe_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_merge_votes mv
                    WHERE mv.proposal_id = $1
                      AND mv.target_proposal_id IS NOT NULL
                      AND EXISTS (
                        SELECT 1
                        FROM proposals target
                        WHERE target.id = mv.target_proposal_id
                          AND target.primary_state = 'active'
                      )
                      AND EXISTS (
                        SELECT 1
                        FROM proposal_merge_relationships r
                        WHERE r.source_proposal_id = mv.proposal_id
                          AND r.target_proposal_id = mv.target_proposal_id
                          AND r.status = 'active'
                      )
                ) AS merge_count
        )
        UPDATE proposals p
        SET
            support_count = counts.support_count,
            not_a_fit_count = counts.not_a_fit_count,
            unclear_count = counts.unclear_count,
            unsafe_count = counts.unsafe_count,
            merge_count = counts.merge_count,
            high_moderation_watch_started_at = CASE
                WHEN counts.unsafe_count >= 8
                  OR (
                    (counts.support_count + counts.not_a_fit_count + counts.unclear_count + counts.unsafe_count + counts.merge_count) > 0
                    AND counts.unsafe_count::numeric
                        / (counts.support_count + counts.not_a_fit_count + counts.unclear_count + counts.unsafe_count + counts.merge_count)::numeric >= 0.50
                  )
                THEN COALESCE(p.high_moderation_watch_started_at, NOW())
                ELSE NULL
            END
        FROM counts
        WHERE p.id = $1
        "#,
    )
    .bind(proposal_id)
    .execute(db)
    .await?;
    Ok(())
}
