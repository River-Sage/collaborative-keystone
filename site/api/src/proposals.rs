use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState, anti_abuse, auth::AuthUser, cycles::open_next_locale_cycle_after_resolution,
    error::AppError, executions::create_execution_record_from_solution, notifications,
    reconsiderations::resolve_cleared_reconsiderations, review_actions::ensure_submit_unlocked,
};

const MAX_REQUIRED_RESOURCE_CATEGORIES: usize = 8;
const MAX_COMPLETION_CRITERIA: usize = 8;
const MAX_RESOURCE_REQUIREMENTS: usize = 64;
const MAX_TITLE_CHARS: usize = 120;
const MAX_SCOPE_CHARS: usize = 500;
const MAX_LONG_TEXT_CHARS: usize = 2000;
const MAX_SOLUTION_FIT_CHARS: usize = 1000;
const MAX_COMPLETION_CRITERION_CHARS: usize = 240;
const MAX_RESOURCE_AMOUNT_CHARS: usize = 64;
const MAX_RESOURCE_UNIT_CHARS: usize = 64;
const MAX_RESOURCE_TARGET_CHARS: usize = 140;
const MAX_NOTE_CHARS: usize = 2000;
const MAX_LINK_CHARS: usize = 2048;
const MAX_TIMESTAMP_CHARS: usize = 64;
const MODERATION_THRESHOLD_HOLD_HOURS: i64 = 24;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateProposalRequest {
    pub board_code: String,
    pub title: String,

    pub problem_description: Option<String>,
    pub affected_scope: Option<String>,
    pub why_it_matters: Option<String>,

    pub action_description: Option<String>,
    pub parent_issue_proposal_id: Option<Uuid>,
    pub required_resource_categories: Option<Value>,
    pub completion_criteria: Option<Value>,
    pub execution_tracking_entries: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct CreateProposalResponse {
    pub ok: bool,
    pub proposal_id: Uuid,
    pub board_code: String,
    pub title: String,
    pub cycle_id: Uuid,
    pub locale_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ListProposalsQuery {
    pub board_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListProposalsResponse {
    pub ok: bool,
    pub proposals: Vec<PublicProposalSummary>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewPoolQuery {
    pub board_code: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ReviewPoolResponse {
    pub ok: bool,
    pub requested_limit: i64,
    pub returned_count: usize,
    pub proposals: Vec<PublicProposalSummary>,
}

#[derive(Debug, Serialize)]
pub struct ReviewQueueResponse {
    pub ok: bool,
    pub proposals: Vec<ReviewQueueItem>,
}

#[derive(Debug, Serialize)]
pub struct CycleOutcomeResponse {
    pub ok: bool,
    pub cycle: CycleSummary,
    pub can_resolve: bool,
    pub results: Vec<CycleResultSummary>,
    pub issue_winner_proposal_id: Option<Uuid>,
    pub solution_winner_proposal_id: Option<Uuid>,
    pub issue_candidates: Vec<CycleOutcomeCandidate>,
    pub solution_candidates: Vec<CycleOutcomeCandidate>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CycleSummary {
    pub id: Uuid,
    pub cycle_number: i32,
    pub starts_at: DateTime<Utc>,
    pub submission_ends_at: DateTime<Utc>,
    pub voting_ends_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CycleResultsResponse {
    pub ok: bool,
    pub results: Vec<CycleResultSummary>,
}

#[derive(Debug, Serialize)]
pub struct ResolveCycleOutcomesResponse {
    pub ok: bool,
    pub cycle: CycleSummary,
    pub results: Vec<CycleResultSummary>,
    pub execution_record_id: Option<Uuid>,
    pub archived_proposal_count: i64,
    pub next_cycle_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct CycleResultSummary {
    pub id: Uuid,
    pub cycle_id: Uuid,
    pub cycle_number: i32,
    pub board_code: String,
    pub result_status: String,
    pub winning_proposal_id: Option<Uuid>,
    pub execution_record_id: Option<Uuid>,
    pub result_snapshot: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub winning_proposal: Option<ProposalSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CycleOutcomeCandidate {
    #[serde(flatten)]
    pub proposal: ProposalSummary,
    pub classification: String,
    pub rank: Option<usize>,
    pub support_ratio: Option<f64>,
    pub unsafe_fraction: Option<f64>,
    pub negative_count: i32,
    pub non_merge_count: i32,
    pub total_count: i32,
}

#[derive(Debug, Serialize)]
pub struct ReviewQueueItem {
    #[serde(flatten)]
    pub proposal: ModeratorReviewProposalSummary,
    pub review_reason: String,
    pub threshold_signal: Option<ThresholdSignalSummary>,
    pub merge_relationships: ProposalMergeRelationships,
}

#[derive(Debug, Serialize)]
pub struct ThresholdSignalSummary {
    pub label: String,
    pub metrics: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ModeratorReviewProposalSummary {
    pub id: Uuid,
    pub board_code: String,
    pub title: String,
    pub primary_state: String,
    pub parent_issue_proposal_id: Option<Uuid>,
    pub merged_into_proposal_id: Option<Uuid>,
    pub archived_reason: Option<String>,
    pub moderation_note: Option<String>,

    pub problem_description: Option<String>,
    pub affected_scope: Option<String>,
    pub why_it_matters: Option<String>,

    pub action_description: Option<String>,
    pub required_resource_categories: Option<Value>,
    pub completion_criteria: Option<Value>,
    pub execution_tracking_entries: Option<Value>,

    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteMergeRequest {
    pub source_proposal_id: Uuid,
    pub target_proposal_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ExecuteMergeResponse {
    pub ok: bool,
    pub source_proposal_id: Uuid,
    pub target_proposal_id: Uuid,
    pub archived_proposal_id: Uuid,
    pub surviving_proposal_id: Uuid,
    pub requested_source_proposal_id: Uuid,
    pub requested_target_proposal_id: Uuid,
    pub archived_total_count: i32,
    pub surviving_total_count: i32,
    pub source_to_target_merge_count: i32,
    pub target_to_source_merge_count: i32,
    pub source_to_target_high_merge_watch: bool,
    pub target_to_source_high_merge_watch: bool,
    pub sentiment_votes_transferred: i64,
    pub sentiment_votes_discarded_same: i64,
    pub sentiment_votes_discarded_conflicting: i64,
    pub source_primary_state: String,
    pub archived_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ModerateArchiveRequest {
    pub proposal_id: Uuid,
    pub archived_reason: String,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModerateArchiveResponse {
    pub ok: bool,
    pub proposal_id: Uuid,
    pub primary_state: String,
    pub archived_reason: String,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModerateFreezeRequest {
    pub proposal_id: Uuid,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModerateFreezeResponse {
    pub ok: bool,
    pub proposal_id: Uuid,
    pub primary_state: String,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModerateUnfreezeRequest {
    pub proposal_id: Uuid,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModerateUnfreezeResponse {
    pub ok: bool,
    pub proposal_id: Uuid,
    pub primary_state: String,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModerateReviewedActiveRequest {
    pub proposal_id: Uuid,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModerateReviewedActiveResponse {
    pub ok: bool,
    pub proposal_id: Uuid,
    pub primary_state: String,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProposalSummary {
    pub id: Uuid,
    pub board_code: String,
    pub title: String,
    pub primary_state: String,
    #[serde(skip_serializing)]
    pub author_user_id: Uuid,
    pub parent_issue_proposal_id: Option<Uuid>,
    pub merged_into_proposal_id: Option<Uuid>,
    pub archived_reason: Option<String>,
    pub moderation_note: Option<String>,

    pub support_count: i32,
    pub not_a_fit_count: i32,
    pub unclear_count: i32,
    pub unsafe_count: i32,
    pub merge_count: i32,

    pub problem_description: Option<String>,
    pub affected_scope: Option<String>,
    pub why_it_matters: Option<String>,

    pub action_description: Option<String>,
    pub required_resource_categories: Option<Value>,
    pub completion_criteria: Option<Value>,
    pub execution_tracking_entries: Option<Value>,

    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing)]
    pub high_moderation_watch_started_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing)]
    pub review_action_count: i64,
    #[serde(skip_serializing)]
    pub cycle_average_review_action_count: f64,
}

#[derive(Debug, Clone)]
pub struct ReviewPoolProposal {
    pub proposal: ProposalSummary,
}

#[derive(Debug, Serialize, Clone)]
pub struct PublicProposalSummary {
    pub id: Uuid,
    pub board_code: String,
    pub title: String,
    pub current_user_is_author: bool,
    pub parent_issue_proposal_id: Option<Uuid>,
    pub merged_into_proposal_id: Option<Uuid>,
    pub is_archived: bool,
    pub archived_reason: Option<String>,
    pub moderation_note: Option<String>,

    pub problem_description: Option<String>,
    pub affected_scope: Option<String>,
    pub why_it_matters: Option<String>,

    pub action_description: Option<String>,
    pub required_resource_categories: Option<Value>,
    pub completion_criteria: Option<Value>,
    pub execution_tracking_entries: Option<Value>,

    pub created_at: DateTime<Utc>,
}

impl ProposalSummary {
    fn to_public(self, current_user_id: Uuid) -> PublicProposalSummary {
        let current_user_is_author = self.author_user_id == current_user_id;

        PublicProposalSummary {
            id: self.id,
            board_code: self.board_code,
            title: self.title,
            current_user_is_author,
            parent_issue_proposal_id: self.parent_issue_proposal_id,
            merged_into_proposal_id: self.merged_into_proposal_id,
            is_archived: self.primary_state == "archived",
            archived_reason: self.archived_reason,
            moderation_note: self.moderation_note,
            problem_description: self.problem_description,
            affected_scope: self.affected_scope,
            why_it_matters: self.why_it_matters,
            action_description: self.action_description,
            required_resource_categories: self.required_resource_categories,
            completion_criteria: self.completion_criteria,
            execution_tracking_entries: self.execution_tracking_entries,
            created_at: self.created_at,
        }
    }

    fn to_moderator_review_summary(&self) -> ModeratorReviewProposalSummary {
        ModeratorReviewProposalSummary {
            id: self.id,
            board_code: self.board_code.clone(),
            title: self.title.clone(),
            primary_state: self.primary_state.clone(),
            parent_issue_proposal_id: self.parent_issue_proposal_id,
            merged_into_proposal_id: self.merged_into_proposal_id,
            archived_reason: self.archived_reason.clone(),
            moderation_note: self.moderation_note.clone(),
            problem_description: self.problem_description.clone(),
            affected_scope: self.affected_scope.clone(),
            why_it_matters: self.why_it_matters.clone(),
            action_description: self.action_description.clone(),
            required_resource_categories: self.required_resource_categories.clone(),
            completion_criteria: self.completion_criteria.clone(),
            execution_tracking_entries: self.execution_tracking_entries.clone(),
            created_at: self.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProposalDetailResponse {
    pub ok: bool,
    pub proposal: PublicProposalSummary,
    pub merge_relationships: ProposalMergeRelationships,
    pub moderator_actions: Vec<ModeratorActionSummary>,
    pub current_user_sentiment_vote: Option<String>,
    pub current_user_merge_vote_present: bool,
    pub current_user_merge_target_proposal_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ModeratorActionSummary {
    pub id: Uuid,
    pub action_type: String,
    pub proposal_id: Uuid,
    pub related_proposal_id: Option<Uuid>,
    pub related_proposal_title: Option<String>,
    pub action_reason: Option<String>,
    pub public_note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProposalMergeRelationships {
    pub outgoing: Vec<ProposalMergeRelationship>,
    pub incoming: Vec<ProposalMergeRelationship>,
}

#[derive(Debug, Serialize)]
pub struct ProposalMergeRelationship {
    pub source_proposal_id: Uuid,
    pub target_proposal_id: Uuid,
    pub source_title: String,
    pub target_title: String,
    pub relationship_status: String,
    pub relationship_created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_to_target_high_merge_watch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_to_source_high_merge_watch: Option<bool>,
    pub note: Option<ProposalMergeRelationshipNote>,
}

#[derive(Debug, Serialize)]
pub struct ProposalMergeRelationshipNote {
    pub difference_type: String,
    pub note_text: String,
    pub created_at: String,
    pub updated_at: String,
}

const STANDARD_REQUIRED_REVIEW_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewBucket {
    LowRatedSalvageable,
    ContestedUnderReviewed,
    MergeHeavy,
    LowExposure,
    Fallback,
}

#[derive(Debug, Clone)]
struct ReviewCandidate {
    proposal: ProposalSummary,
    total_interactions: i32,
    sentiment_total: i32,
    merge_fraction: f64,
    dislike_ratio: f64,
    support_ratio: f64,
    review_action_count: i64,
    cycle_average_review_action_count: f64,
}

#[derive(Clone)]
struct ProposalCounts {
    support: i32,
    not_a_fit: i32,
    unclear: i32,
    unsafe_count: i32,
    merge_count: i32,
}

#[derive(Clone)]
struct MergeProposal {
    id: Uuid,
    title: String,
    board_code: String,
    cycle_id: Uuid,
    locale_id: Uuid,
    primary_state: String,
    frozen_for_review: bool,
    reconsideration_window_open: bool,
    counts: ProposalCounts,
}

struct PairMergeThreshold {
    relationship_exists: bool,
    source_to_target_merge_count: i32,
    target_to_source_merge_count: i32,
    source_to_target_high_merge_watch: bool,
    target_to_source_high_merge_watch: bool,
}

struct SentimentVoteReconciliation {
    transferred: i64,
    discarded_same: i64,
    discarded_conflicting: i64,
}

#[derive(Clone)]
struct ActiveCycle {
    summary: CycleSummary,
    locale_id: Uuid,
    locale_slug: String,
    can_resolve: bool,
}

impl ProposalCounts {
    fn negative_count(&self) -> i32 {
        self.not_a_fit + self.unclear + self.unsafe_count
    }

    fn non_merge_count(&self) -> i32 {
        self.support + self.negative_count()
    }

    fn total_count(&self) -> i32 {
        self.non_merge_count() + self.merge_count
    }

    fn high_merge_watch_for_target(&self, target_merge_count: i32) -> bool {
        self.total_count() >= 20 && fraction_at_least(target_merge_count, self.total_count(), 0.35)
    }

    fn high_moderation_watch(&self) -> bool {
        self.unsafe_count >= 8 || fraction_at_least(self.unsafe_count, self.total_count(), 0.50)
    }

    fn merge_watch(&self) -> bool {
        self.total_count() >= 10 && fraction_at_least(self.merge_count, self.total_count(), 0.20)
    }

    fn moderation_watch(&self) -> bool {
        (self.total_count() >= 8 && fraction_at_least(self.unsafe_count, self.total_count(), 0.20))
            || self.unsafe_count >= 5
            || (self.non_merge_count() >= 10 && self.negative_count() > 8 * self.support.max(1))
    }

    fn to_snapshot(&self) -> Value {
        json!({
            "support_count": self.support,
            "not_a_fit_count": self.not_a_fit,
            "unclear_count": self.unclear,
            "unsafe_count": self.unsafe_count,
            "merge_count": self.merge_count,
            "negative_count": self.negative_count(),
            "non_merge_count": self.non_merge_count(),
            "total_count": self.total_count()
        })
    }
}

fn build_threshold_signal(
    review_reason: &str,
    counts: &ProposalCounts,
) -> Option<ThresholdSignalSummary> {
    match review_reason {
        "high_moderation_hold" | "high_moderation_review" => Some(ThresholdSignalSummary {
            label: "Moderation threshold".to_string(),
            metrics: vec![
                format!("Unsafe flags: {}", counts.unsafe_count),
                format!("Total signals: {}", counts.total_count()),
            ],
        }),
        "moderation_watch_review" => {
            let negative_dominance = counts.non_merge_count() >= 10
                && counts.negative_count() > 8 * counts.support.max(1);

            Some(ThresholdSignalSummary {
                label: "Moderation watch".to_string(),
                metrics: if negative_dominance {
                    vec![
                        format!("Negative signals: {}", counts.negative_count()),
                        format!("Non-duplicate signals: {}", counts.non_merge_count()),
                    ]
                } else {
                    vec![
                        format!("Unsafe flags: {}", counts.unsafe_count),
                        format!("Total signals: {}", counts.total_count()),
                    ]
                },
            })
        }
        "merge_review" => Some(ThresholdSignalSummary {
            label: "Duplicate threshold".to_string(),
            metrics: vec![
                format!("Duplicate flags: {}", counts.merge_count),
                format!("Total signals: {}", counts.total_count()),
            ],
        }),
        _ => None,
    }
}

fn high_moderation_hold_ready(started_at: Option<DateTime<Utc>>) -> bool {
    started_at
        .map(|value| {
            Utc::now().signed_duration_since(value)
                >= Duration::hours(MODERATION_THRESHOLD_HOLD_HOURS)
        })
        .unwrap_or(false)
}

fn require_high_moderation_hold_ready(started_at: Option<DateTime<Utc>>) -> Result<(), AppError> {
    if high_moderation_hold_ready(started_at) {
        return Ok(());
    }

    Err(AppError::Forbidden(format!(
        "Proposal must remain over the high moderation threshold for {MODERATION_THRESHOLD_HOLD_HOURS} hours before moderation action."
    )))
}

pub async fn create_proposal_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
    Json(payload): Json<CreateProposalRequest>,
) -> Result<(StatusCode, Json<CreateProposalResponse>), AppError> {
    auth_user.require_verified()?;

    let board_code = payload.board_code.trim().to_lowercase();
    let title = payload.title.trim().to_string();

    if title.is_empty() {
        return Err(AppError::BadRequest("Title is required.".to_string()));
    }

    validate_text_max_chars(&title, "Title", MAX_TITLE_CHARS)?;
    validate_title_quality(&title)?;

    if board_code != "issue" && board_code != "solution" {
        return Err(AppError::BadRequest(
            "board_code must be either 'issue' or 'solution'.".to_string(),
        ));
    }

    if board_code == "issue" {
        if payload.parent_issue_proposal_id.is_some() {
            return Err(AppError::BadRequest(
                "Issue proposals cannot target a parent issue.".to_string(),
            ));
        }

        require_submission_text(
            &payload.problem_description,
            "Problem description",
            MAX_LONG_TEXT_CHARS,
        )?;
        require_submission_text(&payload.affected_scope, "Affected scope", MAX_SCOPE_CHARS)?;
        require_submission_text(
            &payload.why_it_matters,
            "Why it matters",
            MAX_LONG_TEXT_CHARS,
        )?;
    }

    if board_code == "solution" {
        require_submission_text(
            &payload.action_description,
            "Action description",
            MAX_LONG_TEXT_CHARS,
        )?;
        require_submission_text(
            &payload.why_it_matters,
            "Why this solves it",
            MAX_SOLUTION_FIT_CHARS,
        )?;

        if payload.parent_issue_proposal_id.is_none() {
            return Err(AppError::BadRequest(
                "parent_issue_proposal_id is required for solution proposals.".to_string(),
            ));
        }

        validate_required_resource_categories(payload.required_resource_categories.as_ref())?;
        validate_completion_criteria(payload.completion_criteria.as_ref())?;
        validate_execution_tracking_entries(payload.execution_tracking_entries.as_ref())?;
    }

    let active_cycle = sqlx::query(
        r#"
        SELECT c.id AS cycle_id, c.locale_id
        FROM cycles c
        JOIN locales l ON l.id = c.locale_id
        WHERE l.slug = $1
          AND c.is_active = TRUE
        ORDER BY c.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading active cycle: {}", err);
        AppError::Internal("Failed to load active cycle.".to_string())
    })?;

    let Some(active_cycle) = active_cycle else {
        return Err(AppError::Internal("No active cycle exists.".to_string()));
    };

    let cycle_id: Uuid = active_cycle.try_get("cycle_id").map_err(internal_db_err)?;
    let locale_id: Uuid = active_cycle.try_get("locale_id").map_err(internal_db_err)?;

    if board_code == "solution" {
        let parent_issue_proposal_id = payload.parent_issue_proposal_id.ok_or_else(|| {
            AppError::BadRequest(
                "parent_issue_proposal_id is required for solution proposals.".to_string(),
            )
        })?;

        validate_solution_parent_issue(&state.db, cycle_id, locale_id, parent_issue_proposal_id)
            .await?;
    }

    ensure_submit_unlocked(&state.db, auth_user.user_id, Some(&board_code)).await?;

    let board = sqlx::query(
        r#"
        SELECT id, code
        FROM boards
        WHERE code = $1
          AND is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(&board_code)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading board: {}", err);
        AppError::Internal("Failed to load board.".to_string())
    })?;

    let Some(board) = board else {
        return Err(AppError::BadRequest("Board is not available.".to_string()));
    };

    let board_id: Uuid = board.try_get("id").map_err(internal_db_err)?;
    let board_code: String = board.try_get("code").map_err(internal_db_err)?;

    ensure_distinct_title(&state.db, cycle_id, &board_code, &title).await?;

    let inserted = sqlx::query(
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
            primary_state
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8, $9, $10,
            $11, $12, $13,
            'active'
        )
        RETURNING id, primary_state
        "#,
    )
    .bind(board_id)
    .bind(cycle_id)
    .bind(locale_id)
    .bind(auth_user.user_id)
    .bind(payload.parent_issue_proposal_id)
    .bind(title.clone())
    .bind(trimmed_opt(payload.problem_description))
    .bind(trimmed_opt(payload.affected_scope))
    .bind(trimmed_opt(payload.why_it_matters))
    .bind(trimmed_opt(payload.action_description))
    .bind(payload.required_resource_categories)
    .bind(payload.completion_criteria)
    .bind(payload.execution_tracking_entries)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        error!("database error creating proposal: {}", err);
        AppError::Internal("Failed to create proposal.".to_string())
    })?;

    let response = CreateProposalResponse {
        ok: true,
        proposal_id: inserted.try_get("id").map_err(internal_db_err)?,
        board_code,
        title,
        cycle_id,
        locale_id,
    };

    anti_abuse::record_user_activity(
        &state.db,
        auth_user.user_id,
        "proposal_created",
        Some(response.proposal_id),
        payload.parent_issue_proposal_id,
        &headers,
        json!({
            "board_code": response.board_code,
            "title": response.title
        }),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn validate_solution_parent_issue(
    db: &sqlx::PgPool,
    _active_cycle_id: Uuid,
    active_locale_id: Uuid,
    parent_issue_proposal_id: Uuid,
) -> Result<(), AppError> {
    let latest_winning_issue_id =
        load_latest_published_issue_winner_id(db, active_locale_id).await?;

    if let Some(required_issue_id) = latest_winning_issue_id {
        if parent_issue_proposal_id != required_issue_id {
            return Err(AppError::BadRequest(
                "Solution proposals must target the current winning issue.".to_string(),
            ));
        }

        if !published_winning_issue_target_exists(db, parent_issue_proposal_id, active_locale_id)
            .await?
        {
            return Err(AppError::BadRequest(
                "The current winning issue is not available for solution proposals.".to_string(),
            ));
        }

        return Ok(());
    }

    Err(AppError::BadRequest(
        "Solution proposals open after the first winning issue has been published.".to_string(),
    ))
}

async fn published_winning_issue_target_exists(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    locale_id: Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM proposals p
            JOIN boards b ON b.id = p.board_id
            WHERE p.id = $1
              AND p.locale_id = $2
              AND b.code = 'issue'
              AND p.merged_into_proposal_id IS NULL
              AND (
                p.primary_state = 'active'
                OR (
                    p.primary_state = 'archived'
                    AND p.archived_reason = 'cycle_closed'
                )
              )
        ) AS exists_flag
        "#,
    )
    .bind(proposal_id)
    .bind(locale_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking winning issue target: {}", err);
        AppError::Internal("Failed to validate solution target issue.".to_string())
    })?;

    row.try_get("exists_flag").map_err(internal_db_err)
}

async fn ensure_distinct_title(
    db: &sqlx::PgPool,
    cycle_id: Uuid,
    board_code: &str,
    title: &str,
) -> Result<(), AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM proposals p
            JOIN boards b ON b.id = p.board_id
            WHERE p.cycle_id = $1
              AND b.code = $2
              AND p.primary_state = 'active'
              AND lower(trim(p.title)) = lower(trim($3))
        ) AS exists_flag
        "#,
    )
    .bind(cycle_id)
    .bind(board_code)
    .bind(title)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking duplicate proposal title: {}", err);
        AppError::Internal("Failed to check proposal distinctness.".to_string())
    })?;

    let exists_flag: bool = row.try_get("exists_flag").map_err(internal_db_err)?;
    if exists_flag {
        return Err(AppError::BadRequest(
            "An active proposal with this title already exists on this board.".to_string(),
        ));
    }

    Ok(())
}

async fn load_latest_published_issue_winner_id(
    db: &sqlx::PgPool,
    locale_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT cr.winning_proposal_id
        FROM cycle_results cr
        JOIN cycles c ON c.id = cr.cycle_id
        WHERE cr.locale_id = $1
          AND cr.board_code = 'issue'
          AND cr.result_status = 'resolved'
          AND cr.winning_proposal_id IS NOT NULL
          AND cr.published_at IS NOT NULL
        ORDER BY c.cycle_number DESC, cr.published_at DESC
        LIMIT 1
        "#,
    )
    .bind(locale_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error loading latest issue result: {}", err);
        AppError::Internal("Failed to validate solution target issue.".to_string())
    })?;

    row.map(|row| row.try_get("winning_proposal_id").map_err(internal_db_err))
        .transpose()
}

async fn load_solution_board_target_issue_id(
    db: &sqlx::PgPool,
    cycle: &ActiveCycle,
) -> Result<Option<Uuid>, AppError> {
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
    .bind(cycle.locale_id)
    .bind(cycle.summary.cycle_number)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading solution board target issue: {}",
            err
        );
        AppError::Internal("Failed to load solution board target issue.".to_string())
    })?;

    row.map(|row| row.try_get("winning_proposal_id").map_err(internal_db_err))
        .transpose()
}

fn filter_solution_proposals_for_target(
    proposals: Vec<ProposalSummary>,
    target_issue_id: Option<Uuid>,
) -> Vec<ProposalSummary> {
    match target_issue_id {
        Some(target_issue_id) => proposals
            .into_iter()
            .filter(|proposal| proposal.parent_issue_proposal_id == Some(target_issue_id))
            .collect(),
        None => Vec::new(),
    }
}

pub async fn list_proposals_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<ListProposalsQuery>,
) -> Result<Json<ListProposalsResponse>, AppError> {
    resolve_cleared_reconsiderations(&state.db).await?;

    let board_code_filter = query.board_code.map(|v| v.trim().to_lowercase());

    if let Some(board_code) = board_code_filter.as_deref() {
        if board_code != "issue" && board_code != "solution" && board_code != "archive" {
            return Err(AppError::BadRequest(
                "board_code must be 'issue', 'solution', or 'archive'.".to_string(),
            ));
        }
    }

    let rows = if board_code_filter.as_deref() == Some("archive") {
        fetch_archived_proposals(&state.db, &state.locale.slug).await?
    } else {
        fetch_proposals(
            &state.db,
            &state.locale.slug,
            board_code_filter.as_deref(),
            false,
            None,
        )
        .await?
    };

    let mut proposals = map_proposal_rows(rows)?;
    if board_code_filter.as_deref() == Some("solution") {
        let cycle = load_active_locale_cycle(&state.db, &state.locale.slug).await?;
        let solution_target_issue_id =
            load_solution_board_target_issue_id(&state.db, &cycle).await?;
        proposals = filter_solution_proposals_for_target(proposals, solution_target_issue_id);
    }

    if matches!(
        board_code_filter.as_deref(),
        Some("issue") | Some("solution")
    ) {
        proposals = order_review_feed_proposals(proposals);
    }

    let proposals = proposals
        .into_iter()
        .map(|proposal| proposal.to_public(auth_user.user_id))
        .collect();

    Ok(Json(ListProposalsResponse {
        ok: true,
        proposals,
    }))
}

pub async fn get_proposal_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(proposal_id): Path<Uuid>,
) -> Result<Json<ProposalDetailResponse>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            p.id,
            b.code AS board_code,
            p.title,
            p.primary_state,
            p.author_user_id,
            p.parent_issue_proposal_id,
            p.merged_into_proposal_id,
            p.archived_reason,
            p.moderation_note,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.high_moderation_watch_started_at,
            p.problem_description,
            p.affected_scope,
            p.why_it_matters,
            p.action_description,
            p.required_resource_categories,
            p.completion_criteria,
            p.execution_tracking_entries,
            p.created_at,
            sv.vote_value AS current_user_sentiment_vote,
            CASE WHEN mv.id IS NULL THEN FALSE ELSE TRUE END AS current_user_merge_vote_present,
            mv.target_proposal_id AS current_user_merge_target_proposal_id,
            (
                SELECT COUNT(*)::bigint
                FROM review_actions all_ra
                WHERE all_ra.proposal_id = p.id
                  AND all_ra.cycle_id = p.cycle_id
            ) AS review_action_count,
            (
                SELECT COALESCE(AVG(review_counts.review_count), 0)::float8
                FROM (
                    SELECT COUNT(all_ra2.id)::numeric AS review_count
                    FROM proposals p2
                    JOIN boards b2 ON b2.id = p2.board_id
                    LEFT JOIN review_actions all_ra2
                        ON all_ra2.proposal_id = p2.id
                       AND all_ra2.cycle_id = p2.cycle_id
                    WHERE p2.cycle_id = p.cycle_id
                      AND b2.code = b.code
                      AND p2.primary_state = 'active'
                    GROUP BY p2.id
                ) review_counts
            ) AS cycle_average_review_action_count
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        LEFT JOIN proposal_sentiment_votes sv
          ON sv.proposal_id = p.id
         AND sv.user_id = $2
         AND p.author_user_id <> $2
        LEFT JOIN proposal_merge_votes mv
          ON mv.proposal_id = p.id
         AND mv.user_id = $2
         AND p.author_user_id <> $2
        WHERE p.id = $1
          AND l.slug = $3
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .bind(auth_user.user_id)
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading proposal detail: {}", err);
        AppError::Internal("Failed to load proposal.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let current_user_sentiment_vote = row
        .try_get("current_user_sentiment_vote")
        .map_err(internal_db_err)?;
    let current_user_merge_vote_present = row
        .try_get("current_user_merge_vote_present")
        .map_err(internal_db_err)?;
    let current_user_merge_target_proposal_id = row
        .try_get("current_user_merge_target_proposal_id")
        .map_err(internal_db_err)?;
    let proposal = map_one_proposal_row(row)?;
    let public_proposal = proposal.to_public(auth_user.user_id);
    let merge_relationships = load_merge_relationships(&state.db, proposal_id, false).await?;
    let moderator_actions = load_moderator_actions(&state.db, proposal_id).await?;

    Ok(Json(ProposalDetailResponse {
        ok: true,
        proposal: public_proposal,
        merge_relationships,
        moderator_actions,
        current_user_sentiment_vote,
        current_user_merge_vote_present,
        current_user_merge_target_proposal_id,
    }))
}

pub async fn execute_merge_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ExecuteMergeRequest>,
) -> Result<(StatusCode, Json<ExecuteMergeResponse>), AppError> {
    require_moderator(&auth_user)?;
    resolve_cleared_reconsiderations(&state.db).await?;

    if payload.source_proposal_id == payload.target_proposal_id {
        return Err(AppError::BadRequest(
            "source_proposal_id and target_proposal_id must be different.".to_string(),
        ));
    }

    let mut tx = state.db.begin().await.map_err(|err| {
        error!("database error starting merge transaction: {}", err);
        AppError::Internal("Failed to execute merge.".to_string())
    })?;

    let proposal_rows = sqlx::query(
        r#"
        SELECT
            p.id,
            p.title,
            p.primary_state,
            p.cycle_id,
            p.locale_id,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            b.code AS board_code,
            EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
            ) AS frozen_for_review,
            EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
                  AND (
                    rw.ends_at > NOW()
                    OR p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
            ) AS reconsideration_window_open
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN locales l ON l.id = p.locale_id
        WHERE (p.id = $1 OR p.id = $2)
          AND l.slug = $3
        ORDER BY p.id
        FOR UPDATE
        "#,
    )
    .bind(payload.source_proposal_id)
    .bind(payload.target_proposal_id)
    .bind(&state.locale.slug)
    .fetch_all(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error loading merge proposals: {}", err);
        AppError::Internal("Failed to execute merge.".to_string())
    })?;

    let merge_proposals = proposal_rows
        .into_iter()
        .map(map_merge_proposal_row)
        .collect::<Result<Vec<_>, AppError>>()?;

    let requested_source = merge_proposals
        .iter()
        .find(|proposal| proposal.id == payload.source_proposal_id)
        .cloned()
        .ok_or_else(|| AppError::BadRequest("Source proposal not found.".to_string()))?;
    let requested_target = merge_proposals
        .iter()
        .find(|proposal| proposal.id == payload.target_proposal_id)
        .cloned()
        .ok_or_else(|| AppError::BadRequest("Target proposal not found.".to_string()))?;

    if requested_source.primary_state != "active" || requested_target.primary_state != "active" {
        return Err(AppError::BadRequest(
            "Only active proposals can be merged.".to_string(),
        ));
    }

    if requested_source.frozen_for_review || requested_target.frozen_for_review {
        return Err(AppError::BadRequest(
            "Frozen proposals cannot be merged until unfrozen.".to_string(),
        ));
    }

    if requested_source.reconsideration_window_open || requested_target.reconsideration_window_open
    {
        return Err(AppError::BadRequest(
            "Proposals in reconsideration cannot be merged.".to_string(),
        ));
    }

    if requested_source.board_code != requested_target.board_code
        || requested_source.cycle_id != requested_target.cycle_id
        || requested_source.locale_id != requested_target.locale_id
    {
        return Err(AppError::BadRequest(
            "Merge proposals must share the same board, cycle, and locale.".to_string(),
        ));
    }

    if requested_source.board_code != "issue" && requested_source.board_code != "solution" {
        return Err(AppError::BadRequest(
            "Only issue and solution proposals can be merged.".to_string(),
        ));
    }

    let source_total = requested_source.counts.total_count();
    let target_total = requested_target.counts.total_count();

    if source_total == target_total {
        return Err(AppError::BadRequest(
            "Cannot execute merge while both proposals have equal total vote counts.".to_string(),
        ));
    }

    let (archived_proposal, surviving_proposal, archived_total_count, surviving_total_count) =
        if source_total < target_total {
            (
                requested_source.clone(),
                requested_target.clone(),
                source_total,
                target_total,
            )
        } else {
            (
                requested_target.clone(),
                requested_source.clone(),
                target_total,
                source_total,
            )
        };

    let pair_merge_threshold =
        load_pair_merge_threshold(&mut tx, &requested_source, &requested_target).await?;

    if !pair_merge_threshold.relationship_exists {
        return Err(AppError::BadRequest(
            "An active merge relationship is required before execution.".to_string(),
        ));
    }

    if !pair_merge_threshold.source_to_target_high_merge_watch
        && !pair_merge_threshold.target_to_source_high_merge_watch
    {
        return Err(AppError::Forbidden(
            "Neither proposal has reached the pair-specific merge action threshold with the other proposal as its merge target.".to_string(),
        ));
    }

    let vote_reconciliation =
        reconcile_sentiment_votes_for_merge(&mut tx, archived_proposal.id, surviving_proposal.id)
            .await?;

    sqlx::query(
        r#"
        UPDATE proposals
        SET
            primary_state = 'archived',
            archived_reason = 'merged',
            merged_into_proposal_id = $2,
            moderation_note = NULL
        WHERE id = $1
        "#,
    )
    .bind(archived_proposal.id)
    .bind(surviving_proposal.id)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error applying merge: {}", err);
        AppError::Internal("Failed to execute merge.".to_string())
    })?;

    let relationships_closed = sqlx::query(
        r#"
        UPDATE proposal_merge_relationships
        SET
            status = 'inactive',
            updated_at = NOW()
        WHERE status = 'active'
          AND (
            source_proposal_id = $1
            OR target_proposal_id = $1
          )
        "#,
    )
    .bind(archived_proposal.id)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error closing merge relationships: {}", err);
        AppError::Internal("Failed to execute merge.".to_string())
    })?
    .rows_affected();

    refresh_proposal_vote_counts_tx(&mut tx, archived_proposal.id).await?;
    refresh_proposal_vote_counts_tx(&mut tx, surviving_proposal.id).await?;

    let distinction_note_existed =
        merge_distinction_note_exists(&state.db, archived_proposal.id, surviving_proposal.id)
            .await?;
    let author_notified =
        notifications::merge_watch_author_notified(&state.db, archived_proposal.id).await?
            || notifications::merge_watch_author_notified(&state.db, surviving_proposal.id).await?;

    sqlx::query(
        r#"
        INSERT INTO moderator_actions (
            action_type,
            proposal_id,
            related_proposal_id,
            moderator_user_id,
            action_reason,
            public_note,
            internal_note,
            state_snapshot
        )
        VALUES ('merge', $1, $2, $3, 'merged', NULL, NULL, $4)
        "#,
    )
    .bind(archived_proposal.id)
    .bind(surviving_proposal.id)
    .bind(auth_user.user_id)
    .bind(json!({
        "requested_source_proposal_id": payload.source_proposal_id,
        "requested_target_proposal_id": payload.target_proposal_id,
        "archived_proposal_id": archived_proposal.id,
        "archived_proposal_title": archived_proposal.title,
        "surviving_proposal_id": surviving_proposal.id,
        "surviving_proposal_title": surviving_proposal.title,
        "archived_previous_state": archived_proposal.primary_state,
        "surviving_state": surviving_proposal.primary_state,
        "archived_vote_counts": archived_proposal.counts.to_snapshot(),
        "surviving_vote_counts": surviving_proposal.counts.to_snapshot(),
        "archived_total_count": archived_total_count,
        "surviving_total_count": surviving_total_count,
        "source_to_target_merge_count": pair_merge_threshold.source_to_target_merge_count,
        "target_to_source_merge_count": pair_merge_threshold.target_to_source_merge_count,
        "source_to_target_high_merge_watch": pair_merge_threshold.source_to_target_high_merge_watch,
        "target_to_source_high_merge_watch": pair_merge_threshold.target_to_source_high_merge_watch,
        "sentiment_votes_transferred": vote_reconciliation.transferred,
        "sentiment_votes_discarded_same": vote_reconciliation.discarded_same,
        "sentiment_votes_discarded_conflicting": vote_reconciliation.discarded_conflicting,
        "merge_votes_preserved_on_archived_proposal": archived_proposal.counts.merge_count,
        "relationships_closed": relationships_closed,
        "distinction_note_existed": distinction_note_existed,
        "author_notified": author_notified
    }))
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error inserting merge audit action: {}", err);
        AppError::Internal("Failed to log moderator action.".to_string())
    })?;

    tx.commit().await.map_err(|err| {
        error!("database error committing merge transaction: {}", err);
        AppError::Internal("Failed to execute merge.".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(ExecuteMergeResponse {
            ok: true,
            source_proposal_id: archived_proposal.id,
            target_proposal_id: surviving_proposal.id,
            archived_proposal_id: archived_proposal.id,
            surviving_proposal_id: surviving_proposal.id,
            requested_source_proposal_id: payload.source_proposal_id,
            requested_target_proposal_id: payload.target_proposal_id,
            archived_total_count,
            surviving_total_count,
            source_to_target_merge_count: pair_merge_threshold.source_to_target_merge_count,
            target_to_source_merge_count: pair_merge_threshold.target_to_source_merge_count,
            source_to_target_high_merge_watch: pair_merge_threshold
                .source_to_target_high_merge_watch,
            target_to_source_high_merge_watch: pair_merge_threshold
                .target_to_source_high_merge_watch,
            sentiment_votes_transferred: vote_reconciliation.transferred,
            sentiment_votes_discarded_same: vote_reconciliation.discarded_same,
            sentiment_votes_discarded_conflicting: vote_reconciliation.discarded_conflicting,
            source_primary_state: "archived".to_string(),
            archived_reason: "merged".to_string(),
        }),
    ))
}

pub async fn moderate_archive_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ModerateArchiveRequest>,
) -> Result<(StatusCode, Json<ModerateArchiveResponse>), AppError> {
    require_moderator(&auth_user)?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let archived_reason = payload.archived_reason.trim().to_lowercase();

    if !is_valid_archive_reason(&archived_reason) {
        return Err(AppError::BadRequest(
            "archived_reason must be one of: duplicate, unsafe_illegal_deceptive, spam_abuse, irrelevant, minimum_quality, superseded, moderation, manual_archive, not_a_fit."
                .to_string(),
        ));
    }

    let note = payload
        .moderation_note
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let current = sqlx::query(
        r#"
        SELECT
            p.id,
            p.primary_state,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.high_moderation_watch_started_at,
            EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
            ) AS frozen_for_review,
            EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
                  AND (
                    rw.ends_at > NOW()
                    OR p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
            ) AS reconsideration_window_open
        FROM proposals p
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = $2
        LIMIT 1
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading proposal before archive: {}", err);
        AppError::Internal("Failed to archive proposal.".to_string())
    })?;

    let Some(current) = current else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let current_state: String = current.try_get("primary_state").map_err(internal_db_err)?;
    if current_state != "active" {
        return Err(AppError::BadRequest("Proposal is not active.".to_string()));
    }

    let reconsideration_window_open: bool = current
        .try_get("reconsideration_window_open")
        .map_err(internal_db_err)?;
    if reconsideration_window_open {
        return Err(AppError::BadRequest(
            "Use the reconsideration resolution flow for proposals in reconsideration.".to_string(),
        ));
    }

    let current_counts = proposal_counts_from_row(&current)?;
    let frozen_for_review: bool = current
        .try_get("frozen_for_review")
        .map_err(internal_db_err)?;
    if !frozen_for_review && !current_counts.high_moderation_watch() {
        return Err(AppError::Forbidden(
            "Proposal has not reached the moderation action threshold.".to_string(),
        ));
    }
    if !frozen_for_review {
        let threshold_started_at: Option<DateTime<Utc>> = current
            .try_get("high_moderation_watch_started_at")
            .map_err(internal_db_err)?;
        require_high_moderation_hold_ready(threshold_started_at)?;
    }

    let row = sqlx::query(
        r#"
        UPDATE proposals
        SET
            primary_state = 'archived',
            archived_reason = $2,
            moderation_note = $3,
            merged_into_proposal_id = NULL
        WHERE id = $1
          AND primary_state = 'active'
        RETURNING id, primary_state, archived_reason, moderation_note
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&archived_reason)
    .bind(&note)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error applying moderation archive: {}", err);
        AppError::Internal("Failed to archive proposal.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Proposal not found or is not active.".to_string(),
        ));
    };

    clear_frozen_for_review_flag(
        &state.db,
        payload.proposal_id,
        Some(auth_user.user_id),
        Some("archived_after_review"),
    )
    .await?;

    insert_moderator_action(
        &state.db,
        "archive",
        payload.proposal_id,
        None,
        auth_user.user_id,
        Some(&archived_reason),
        note.as_deref(),
        None,
        json!({
            "previous_state": current_state,
            "was_frozen_for_review": frozen_for_review,
            "archived_reason": archived_reason,
            "moderation_note_present": note.is_some(),
            "vote_counts": current_counts.to_snapshot()
        }),
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(ModerateArchiveResponse {
            ok: true,
            proposal_id: row.try_get("id").map_err(internal_db_err)?,
            primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
            archived_reason: row.try_get("archived_reason").map_err(internal_db_err)?,
            moderation_note: row.try_get("moderation_note").map_err(internal_db_err)?,
        }),
    ))
}

pub async fn moderate_freeze_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ModerateFreezeRequest>,
) -> Result<(StatusCode, Json<ModerateFreezeResponse>), AppError> {
    require_moderator(&auth_user)?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let note = payload
        .moderation_note
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let current = sqlx::query(
        r#"
        SELECT
            p.id,
            p.primary_state,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.high_moderation_watch_started_at,
            EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
            ) AS frozen_for_review,
            EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
                  AND (
                    rw.ends_at > NOW()
                    OR p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
            ) AS reconsideration_window_open
        FROM proposals p
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = $2
        LIMIT 1
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading proposal before freeze: {}", err);
        AppError::Internal("Failed to freeze proposal.".to_string())
    })?;

    let Some(current) = current else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let current_state: String = current.try_get("primary_state").map_err(internal_db_err)?;
    if current_state != "active" {
        return Err(AppError::BadRequest(
            "Only active proposals can be frozen.".to_string(),
        ));
    }

    let reconsideration_window_open: bool = current
        .try_get("reconsideration_window_open")
        .map_err(internal_db_err)?;
    if reconsideration_window_open {
        return Err(AppError::BadRequest(
            "Use the reconsideration resolution flow for proposals in reconsideration.".to_string(),
        ));
    }

    let frozen_for_review: bool = current
        .try_get("frozen_for_review")
        .map_err(internal_db_err)?;
    if frozen_for_review {
        return Err(AppError::BadRequest(
            "Proposal is already frozen for review.".to_string(),
        ));
    }

    let current_counts = proposal_counts_from_row(&current)?;
    if !current_counts.high_moderation_watch() {
        return Err(AppError::Forbidden(
            "Proposal has not reached the moderation action threshold.".to_string(),
        ));
    }
    let threshold_started_at: Option<DateTime<Utc>> = current
        .try_get("high_moderation_watch_started_at")
        .map_err(internal_db_err)?;
    require_high_moderation_hold_ready(threshold_started_at)?;

    let row = sqlx::query(
        r#"
        UPDATE proposals
        SET
            moderation_note = $2,
            merged_into_proposal_id = NULL
        WHERE id = $1
          AND primary_state = 'active'
        RETURNING id, primary_state, moderation_note
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&note)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error applying moderation freeze: {}", err);
        AppError::Internal("Failed to freeze proposal.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Proposal not found or is not active.".to_string(),
        ));
    };

    insert_frozen_for_review_flag(
        &state.db,
        payload.proposal_id,
        Some(auth_user.user_id),
        Some("moderation_freeze"),
    )
    .await?;

    insert_moderator_action(
        &state.db,
        "freeze",
        payload.proposal_id,
        None,
        auth_user.user_id,
        Some("moderation_freeze"),
        note.as_deref(),
        None,
        json!({
            "previous_state": current_state,
            "moderation_note_present": note.is_some(),
            "vote_counts": current_counts.to_snapshot()
        }),
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(ModerateFreezeResponse {
            ok: true,
            proposal_id: row.try_get("id").map_err(internal_db_err)?,
            primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
            moderation_note: row.try_get("moderation_note").map_err(internal_db_err)?,
        }),
    ))
}

pub async fn moderate_unfreeze_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ModerateUnfreezeRequest>,
) -> Result<(StatusCode, Json<ModerateUnfreezeResponse>), AppError> {
    require_moderator(&auth_user)?;

    let note = payload
        .moderation_note
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let current = sqlx::query(
        r#"
        SELECT
            p.id,
            p.primary_state,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.high_moderation_watch_started_at,
            EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
            ) AS frozen_for_review
        FROM proposals p
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = $2
        LIMIT 1
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading proposal before unfreeze: {}", err);
        AppError::Internal("Failed to unfreeze proposal.".to_string())
    })?;

    let Some(current) = current else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let current_state: String = current.try_get("primary_state").map_err(internal_db_err)?;
    let frozen_for_review: bool = current
        .try_get("frozen_for_review")
        .map_err(internal_db_err)?;
    if current_state != "active" || !frozen_for_review {
        return Err(AppError::BadRequest(
            "Only frozen proposals can be unfrozen.".to_string(),
        ));
    }

    let current_counts = proposal_counts_from_row(&current)?;
    let row = sqlx::query(
        r#"
        UPDATE proposals
        SET
            archived_reason = NULL,
            moderation_note = $2,
            merged_into_proposal_id = NULL
        WHERE id = $1
          AND primary_state = 'active'
        RETURNING id, primary_state, moderation_note
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&note)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error applying moderation unfreeze: {}", err);
        AppError::Internal("Failed to unfreeze proposal.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Proposal not found or is not frozen.".to_string(),
        ));
    };

    clear_frozen_for_review_flag(
        &state.db,
        payload.proposal_id,
        Some(auth_user.user_id),
        Some("moderation_unfreeze"),
    )
    .await?;

    insert_moderator_action(
        &state.db,
        "unfreeze",
        payload.proposal_id,
        None,
        auth_user.user_id,
        Some("moderation_unfreeze"),
        note.as_deref(),
        None,
        json!({
            "previous_state": current_state,
            "moderation_note_present": note.is_some(),
            "vote_counts": current_counts.to_snapshot()
        }),
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(ModerateUnfreezeResponse {
            ok: true,
            proposal_id: row.try_get("id").map_err(internal_db_err)?,
            primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
            moderation_note: row.try_get("moderation_note").map_err(internal_db_err)?,
        }),
    ))
}

pub async fn moderate_reviewed_active_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ModerateReviewedActiveRequest>,
) -> Result<(StatusCode, Json<ModerateReviewedActiveResponse>), AppError> {
    require_moderator(&auth_user)?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let note = required_moderation_note(payload.moderation_note.as_ref())?;

    let current = sqlx::query(
        r#"
        SELECT
            p.id,
            p.primary_state,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.high_moderation_watch_started_at,
            EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
            ) AS frozen_for_review,
            EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
                  AND (
                    rw.ends_at > NOW()
                    OR p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
            ) AS reconsideration_window_open
        FROM proposals p
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = $2
        LIMIT 1
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading proposal before active review: {}", err);
        AppError::Internal("Failed to mark proposal reviewed.".to_string())
    })?;

    let Some(current) = current else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let current_state: String = current.try_get("primary_state").map_err(internal_db_err)?;
    if current_state != "active" {
        return Err(AppError::BadRequest(
            "Only active proposals can be marked reviewed active.".to_string(),
        ));
    }

    let reconsideration_window_open: bool = current
        .try_get("reconsideration_window_open")
        .map_err(internal_db_err)?;
    if reconsideration_window_open {
        return Err(AppError::BadRequest(
            "Use the reconsideration resolution flow for proposals in reconsideration.".to_string(),
        ));
    }

    let current_counts = proposal_counts_from_row(&current)?;
    let frozen_for_review: bool = current
        .try_get("frozen_for_review")
        .map_err(internal_db_err)?;
    if !frozen_for_review && !current_counts.high_moderation_watch() {
        return Err(AppError::Forbidden(
            "Proposal has not reached the moderation action threshold.".to_string(),
        ));
    }
    if !frozen_for_review {
        let threshold_started_at: Option<DateTime<Utc>> = current
            .try_get("high_moderation_watch_started_at")
            .map_err(internal_db_err)?;
        require_high_moderation_hold_ready(threshold_started_at)?;
    }

    let row = sqlx::query(
        r#"
        UPDATE proposals
        SET
            archived_reason = NULL,
            moderation_note = $2,
            merged_into_proposal_id = NULL
        WHERE id = $1
          AND primary_state = 'active'
        RETURNING id, primary_state, moderation_note
        "#,
    )
    .bind(payload.proposal_id)
    .bind(&note)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error applying active review: {}", err);
        AppError::Internal("Failed to mark proposal reviewed.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Proposal not found or is not active.".to_string(),
        ));
    };

    if frozen_for_review {
        clear_frozen_for_review_flag(
            &state.db,
            payload.proposal_id,
            Some(auth_user.user_id),
            Some("reviewed_active"),
        )
        .await?;
    }

    insert_moderator_action(
        &state.db,
        "moderator_note",
        payload.proposal_id,
        None,
        auth_user.user_id,
        Some("reviewed_active"),
        Some(&note),
        None,
        json!({
            "previous_state": current_state,
            "was_frozen_for_review": frozen_for_review,
            "vote_counts": current_counts.to_snapshot()
        }),
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(ModerateReviewedActiveResponse {
            ok: true,
            proposal_id: row.try_get("id").map_err(internal_db_err)?,
            primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
            moderation_note: row.try_get("moderation_note").map_err(internal_db_err)?,
        }),
    ))
}

pub async fn review_pool_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<ReviewPoolQuery>,
) -> Result<Json<ReviewPoolResponse>, AppError> {
    resolve_cleared_reconsiderations(&state.db).await?;

    let board_code_filter = query.board_code.map(|v| v.trim().to_lowercase());

    if let Some(board_code) = board_code_filter.as_deref() {
        if board_code != "issue" && board_code != "solution" {
            return Err(AppError::BadRequest(
                "board_code must be 'issue' or 'solution'.".to_string(),
            ));
        }
    }

    let requested_limit = query.limit.unwrap_or(1).clamp(1, 4);

    let rows = fetch_reviewable_proposals_for_user(
        &state.db,
        &state.locale.slug,
        auth_user.user_id,
        board_code_filter.as_deref(),
    )
    .await?;
    let proposals = map_proposal_rows(rows)?;
    let candidates = build_review_candidates(proposals, true);

    let selected = select_review_pool(candidates, requested_limit as usize);
    let returned_count = selected.len();
    let proposals = selected
        .into_iter()
        .map(|item| item.proposal.to_public(auth_user.user_id))
        .collect();

    Ok(Json(ReviewPoolResponse {
        ok: true,
        requested_limit,
        returned_count,
        proposals,
    }))
}

pub async fn review_queue_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ReviewQueueResponse>, AppError> {
    require_moderator(&auth_user)?;

    let rows = sqlx::query(
        r#"
        SELECT
            p.id,
            b.code AS board_code,
            p.title,
            p.primary_state,
            p.author_user_id,
            p.parent_issue_proposal_id,
            p.merged_into_proposal_id,
            p.archived_reason,
            p.moderation_note,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.high_moderation_watch_started_at,
            p.problem_description,
            p.affected_scope,
            p.why_it_matters,
            p.action_description,
            p.required_resource_categories,
            p.completion_criteria,
            p.execution_tracking_entries,
            p.created_at,
            (
                SELECT COUNT(*)::bigint
                FROM review_actions all_ra
                WHERE all_ra.proposal_id = p.id
                  AND all_ra.cycle_id = p.cycle_id
            ) AS review_action_count,
            (
                SELECT COALESCE(AVG(review_counts.review_count), 0)::float8
                FROM (
                    SELECT COUNT(all_ra2.id)::numeric AS review_count
                    FROM proposals p2
                    JOIN boards b2 ON b2.id = p2.board_id
                    LEFT JOIN review_actions all_ra2
                        ON all_ra2.proposal_id = p2.id
                       AND all_ra2.cycle_id = p2.cycle_id
                    WHERE p2.cycle_id = p.cycle_id
                      AND b2.code = b.code
                      AND p2.primary_state = 'active'
                    GROUP BY p2.id
                ) review_counts
            ) AS cycle_average_review_action_count
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE l.slug = $1
          AND c.is_active = TRUE
          AND p.primary_state = 'active'
          AND NOT EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
              )
          AND (
                EXISTS (
                    SELECT 1
                    FROM proposal_watch_flags wf
                    WHERE wf.proposal_id = p.id
                      AND wf.flag_code = 'frozen_for_review'
                      AND wf.cleared_at IS NULL
                )
                OR
                (
                    (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) >= 10
                    AND p.merge_count::numeric
                        / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.20
                )
                OR p.unsafe_count >= 8
                OR (
                    (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                    AND p.unsafe_count::numeric
                        / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                )
                OR (
                    (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) >= 8
                    AND p.unsafe_count::numeric
                        / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.20
                )
                OR p.unsafe_count >= 5
                OR (
                    (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count) >= 10
                    AND (p.not_a_fit_count + p.unclear_count + p.unsafe_count)
                        > 8 * GREATEST(p.support_count, 1)
                )
              )
          AND (
                EXISTS (
                    SELECT 1
                    FROM proposal_watch_flags wf
                    WHERE wf.proposal_id = p.id
                      AND wf.flag_code = 'frozen_for_review'
                      AND wf.cleared_at IS NULL
                )
                OR NOT EXISTS (
                    SELECT 1
                    FROM moderator_actions ma
                    WHERE ma.proposal_id = p.id
                      AND ma.action_type = 'moderator_note'
                      AND ma.action_reason = 'reviewed_active'
                      AND ma.created_at >= GREATEST(
                        p.created_at,
                        COALESCE((
                            SELECT MAX(sv.updated_at)
                            FROM proposal_sentiment_votes sv
                            WHERE sv.proposal_id = p.id
                        ), p.created_at),
                        COALESCE((
                            SELECT MAX(mv.updated_at)
                            FROM proposal_merge_votes mv
                            WHERE mv.proposal_id = p.id
                        ), p.created_at),
                        COALESCE((
                            SELECT MAX(r.updated_at)
                            FROM proposal_merge_relationships r
                            WHERE r.status = 'active'
                              AND (
                                r.source_proposal_id = p.id
                                OR r.target_proposal_id = p.id
                              )
                        ), p.created_at)
                      )
                )
              )
        ORDER BY p.merge_count DESC, p.not_a_fit_count DESC, p.unsafe_count DESC, p.created_at ASC
        "#,
    )
    .bind(&state.locale.slug)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading review queue: {}", err);
        AppError::Internal("Failed to load review queue.".to_string())
    })?;

    let proposals = map_proposal_rows(rows)?;
    let mut items = Vec::new();

    for proposal in proposals {
        let relationships = load_merge_relationships(&state.db, proposal.id, true).await?;
        let frozen_for_review =
            proposal_has_active_watch_flag(&state.db, proposal.id, "frozen_for_review").await?;
        let counts = proposal_counts_from_summary(&proposal);

        let review_reason = if frozen_for_review {
            "frozen_review".to_string()
        } else if counts.high_moderation_watch()
            && high_moderation_hold_ready(proposal.high_moderation_watch_started_at)
        {
            "high_moderation_review".to_string()
        } else if counts.high_moderation_watch() {
            "high_moderation_hold".to_string()
        } else if counts.moderation_watch() {
            "moderation_watch_review".to_string()
        } else if counts.merge_watch() {
            "merge_review".to_string()
        } else {
            "relationship_review".to_string()
        };
        let threshold_signal = build_threshold_signal(&review_reason, &counts);

        items.push(ReviewQueueItem {
            proposal: proposal.to_moderator_review_summary(),
            review_reason,
            threshold_signal,
            merge_relationships: relationships,
        });
    }

    Ok(Json(ReviewQueueResponse {
        ok: true,
        proposals: items,
    }))
}

pub async fn current_cycle_outcome_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<CycleOutcomeResponse>, AppError> {
    require_moderator(&auth_user)?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let cycle = load_active_locale_cycle(&state.db, &state.locale.slug).await?;
    let issue_rows = fetch_proposals_for_cycle(
        &state.db,
        &state.locale.slug,
        cycle.summary.id,
        Some("issue"),
    )
    .await?;
    let solution_rows = fetch_proposals_for_cycle(
        &state.db,
        &state.locale.slug,
        cycle.summary.id,
        Some("solution"),
    )
    .await?;
    let solution_target_issue_id = load_solution_board_target_issue_id(&state.db, &cycle).await?;

    let issue_candidates = build_cycle_outcome_candidates(map_proposal_rows(issue_rows)?);
    let solution_candidates = build_cycle_outcome_candidates(filter_solution_proposals_for_target(
        map_proposal_rows(solution_rows)?,
        solution_target_issue_id,
    ));
    let results =
        load_cycle_results(&state.db, &state.locale.slug, Some(cycle.summary.id), false).await?;

    let issue_winner_proposal_id = issue_candidates
        .iter()
        .find(|candidate| candidate.rank == Some(1))
        .map(|candidate| candidate.proposal.id);
    let solution_winner_proposal_id = solution_candidates
        .iter()
        .find(|candidate| candidate.rank == Some(1))
        .map(|candidate| candidate.proposal.id);

    Ok(Json(CycleOutcomeResponse {
        ok: true,
        cycle: cycle.summary,
        can_resolve: cycle.can_resolve,
        results,
        issue_winner_proposal_id,
        solution_winner_proposal_id,
        issue_candidates,
        solution_candidates,
    }))
}

pub async fn published_cycle_results_handler(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
) -> Result<Json<CycleResultsResponse>, AppError> {
    let results = load_cycle_results(&state.db, &state.locale.slug, None, true).await?;

    Ok(Json(CycleResultsResponse { ok: true, results }))
}

pub async fn resolve_current_cycle_outcomes_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<(StatusCode, Json<ResolveCycleOutcomesResponse>), AppError> {
    require_moderator(&auth_user)?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let cycle = load_active_locale_cycle(&state.db, &state.locale.slug).await?;
    if !cycle.can_resolve {
        return Err(AppError::Forbidden(
            "Current cycle cannot be resolved until voting has closed.".to_string(),
        ));
    }

    let issue_result = resolve_cycle_board(&state.db, &cycle, "issue", auth_user.user_id).await?;
    let solution_result =
        resolve_cycle_board(&state.db, &cycle, "solution", auth_user.user_id).await?;

    let execution_record_id = solution_result.execution_record_id;
    let results = vec![issue_result, solution_result];
    let archived_proposal_count =
        archive_active_cycle_proposals(&state.db, &cycle, auth_user.user_id).await?;
    let next_cycle_id =
        open_next_locale_cycle_after_resolution(&state.db, cycle.summary.id, &state.locale.slug)
            .await
            .map_err(|err| {
                error!("database error opening next cycle: {}", err);
                AppError::Internal("Failed to open the next cycle.".to_string())
            })?;

    Ok((
        StatusCode::OK,
        Json(ResolveCycleOutcomesResponse {
            ok: true,
            cycle: cycle.summary,
            results,
            execution_record_id,
            archived_proposal_count,
            next_cycle_id,
        }),
    ))
}

async fn archive_active_cycle_proposals(
    db: &sqlx::PgPool,
    cycle: &ActiveCycle,
    moderator_user_id: Uuid,
) -> Result<i64, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            p.id,
            b.code AS board_code,
            p.title,
            p.primary_state,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
            ) AS frozen_for_review
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        WHERE p.cycle_id = $1
          AND p.primary_state = 'active'
          AND b.code IN ('issue', 'solution')
        ORDER BY p.created_at ASC
        "#,
    )
    .bind(cycle.summary.id)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading proposals for cycle close archival: {}",
            err
        );
        AppError::Internal("Failed to archive cycle proposals.".to_string())
    })?;

    if rows.is_empty() {
        return Ok(0);
    }

    sqlx::query(
        r#"
        UPDATE proposals
        SET
            primary_state = 'archived',
            archived_reason = 'cycle_closed',
            moderation_note = COALESCE(moderation_note, 'Archived automatically at cycle close.'),
            merged_into_proposal_id = NULL
        WHERE cycle_id = $1
          AND primary_state = 'active'
          AND board_id IN (
            SELECT id
            FROM boards
            WHERE code IN ('issue', 'solution')
          )
        "#,
    )
    .bind(cycle.summary.id)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error archiving proposals at cycle close: {}", err);
        AppError::Internal("Failed to archive cycle proposals.".to_string())
    })?;

    for row in &rows {
        let proposal_id: Uuid = row.try_get("id").map_err(internal_db_err)?;
        let board_code: String = row.try_get("board_code").map_err(internal_db_err)?;
        let title: String = row.try_get("title").map_err(internal_db_err)?;
        let previous_state: String = row.try_get("primary_state").map_err(internal_db_err)?;
        let frozen_for_review: bool = row.try_get("frozen_for_review").map_err(internal_db_err)?;
        let counts = proposal_counts_from_row(row)?;

        if frozen_for_review {
            clear_frozen_for_review_flag(
                db,
                proposal_id,
                Some(moderator_user_id),
                Some("cycle_closed"),
            )
            .await?;
        }

        insert_moderator_action(
            db,
            "archive",
            proposal_id,
            None,
            moderator_user_id,
            Some("cycle_closed"),
            Some("Archived automatically at cycle close."),
            None,
            json!({
                "cycle_id": cycle.summary.id,
                "cycle_number": cycle.summary.cycle_number,
                "board_code": board_code,
                "proposal_title": title,
                "previous_state": previous_state,
                "was_frozen_for_review": frozen_for_review,
                "archived_reason": "cycle_closed",
                "vote_counts": counts.to_snapshot()
            }),
        )
        .await?;
    }

    Ok(rows.len() as i64)
}

async fn fetch_proposals(
    db: &sqlx::PgPool,
    locale_slug: &str,
    board_code_filter: Option<&str>,
    least_exposed_first: bool,
    limit: Option<i64>,
) -> Result<Vec<sqlx::postgres::PgRow>, AppError> {
    let order_clause = if least_exposed_first {
        r#"
        ORDER BY
            (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) ASC,
            p.created_at ASC
        "#
    } else {
        r#"
        ORDER BY p.created_at DESC
        "#
    };

    let limit_clause = if limit.is_some() { "LIMIT $3" } else { "" };

    let sql = format!(
        r#"
        SELECT
            p.id,
            b.code AS board_code,
            p.title,
            p.primary_state,
            p.author_user_id,
            p.parent_issue_proposal_id,
            p.merged_into_proposal_id,
            p.archived_reason,
            p.moderation_note,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.problem_description,
            p.affected_scope,
            p.why_it_matters,
            p.action_description,
            p.required_resource_categories,
            p.completion_criteria,
            p.execution_tracking_entries,
            p.created_at,
            (
                SELECT COUNT(*)::bigint
                FROM review_actions all_ra
                WHERE all_ra.proposal_id = p.id
                  AND all_ra.cycle_id = p.cycle_id
            ) AS review_action_count,
            (
                SELECT COALESCE(AVG(review_counts.review_count), 0)::float8
                FROM (
                    SELECT COUNT(all_ra2.id)::numeric AS review_count
                    FROM proposals p2
                    JOIN boards b2 ON b2.id = p2.board_id
                    LEFT JOIN review_actions all_ra2
                        ON all_ra2.proposal_id = p2.id
                       AND all_ra2.cycle_id = p2.cycle_id
                    WHERE p2.cycle_id = p.cycle_id
                      AND b2.code = b.code
                      AND p2.primary_state = 'active'
                    GROUP BY p2.id
                ) review_counts
            ) AS cycle_average_review_action_count
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE l.slug = $1
          AND c.is_active = TRUE
          AND p.primary_state = 'active'
          AND ($2::text IS NULL OR b.code = $2)
          AND NOT EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
              )
          AND NOT EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
                  AND rw.ends_at <= NOW()
                  AND (
                    p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
              )
        {order_clause}
        {limit_clause}
        "#
    );

    let mut query_builder = sqlx::query(&sql).bind(locale_slug).bind(board_code_filter);

    if let Some(limit) = limit {
        query_builder = query_builder.bind(limit);
    }

    query_builder.fetch_all(db).await.map_err(|err| {
        error!("database error fetching proposals: {}", err);
        AppError::Internal("Failed to load proposals.".to_string())
    })
}

async fn fetch_proposals_for_cycle(
    db: &sqlx::PgPool,
    locale_slug: &str,
    cycle_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<Vec<sqlx::postgres::PgRow>, AppError> {
    sqlx::query(
        r#"
        SELECT
            p.id,
            b.code AS board_code,
            p.title,
            p.primary_state,
            p.author_user_id,
            p.parent_issue_proposal_id,
            p.merged_into_proposal_id,
            p.archived_reason,
            p.moderation_note,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.problem_description,
            p.affected_scope,
            p.why_it_matters,
            p.action_description,
            p.required_resource_categories,
            p.completion_criteria,
            p.execution_tracking_entries,
            p.created_at,
            (
                SELECT COUNT(*)::bigint
                FROM review_actions all_ra
                WHERE all_ra.proposal_id = p.id
                  AND all_ra.cycle_id = p.cycle_id
            ) AS review_action_count,
            (
                SELECT COALESCE(AVG(review_counts.review_count), 0)::float8
                FROM (
                    SELECT COUNT(all_ra2.id)::numeric AS review_count
                    FROM proposals p2
                    JOIN boards b2 ON b2.id = p2.board_id
                    LEFT JOIN review_actions all_ra2
                        ON all_ra2.proposal_id = p2.id
                       AND all_ra2.cycle_id = p2.cycle_id
                    WHERE p2.cycle_id = p.cycle_id
                      AND b2.code = b.code
                      AND p2.primary_state = 'active'
                    GROUP BY p2.id
                ) review_counts
            ) AS cycle_average_review_action_count
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN locales l ON l.id = p.locale_id
        WHERE l.slug = $1
          AND p.cycle_id = $2
          AND p.primary_state = 'active'
          AND ($3::text IS NULL OR b.code = $3)
          AND NOT EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
              )
          AND NOT EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
                  AND rw.ends_at <= NOW()
                  AND (
                    p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
              )
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(locale_slug)
    .bind(cycle_id)
    .bind(board_code_filter)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!("database error fetching cycle proposals: {}", err);
        AppError::Internal("Failed to load cycle proposals.".to_string())
    })
}

async fn load_active_locale_cycle(
    db: &sqlx::PgPool,
    locale_slug: &str,
) -> Result<ActiveCycle, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            c.id,
            c.locale_id,
            l.slug AS locale_slug,
            c.cycle_number,
            c.starts_at,
            c.submission_ends_at,
            c.voting_ends_at,
            (c.voting_ends_at <= NOW()) AS can_resolve
        FROM cycles c
        JOIN locales l ON l.id = c.locale_id
        WHERE l.slug = $1
          AND c.is_active = TRUE
        ORDER BY c.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(locale_slug)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error loading active cycle: {}", err);
        AppError::Internal("Failed to load active cycle.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::Internal("No active cycle exists.".to_string()));
    };

    Ok(ActiveCycle {
        summary: CycleSummary {
            id: row.try_get("id").map_err(internal_db_err)?,
            cycle_number: row.try_get("cycle_number").map_err(internal_db_err)?,
            starts_at: row.try_get("starts_at").map_err(internal_db_err)?,
            submission_ends_at: row.try_get("submission_ends_at").map_err(internal_db_err)?,
            voting_ends_at: row.try_get("voting_ends_at").map_err(internal_db_err)?,
        },
        locale_id: row.try_get("locale_id").map_err(internal_db_err)?,
        locale_slug: row.try_get("locale_slug").map_err(internal_db_err)?,
        can_resolve: row.try_get("can_resolve").map_err(internal_db_err)?,
    })
}

async fn resolve_cycle_board(
    db: &sqlx::PgPool,
    cycle: &ActiveCycle,
    board_code: &str,
    moderator_user_id: Uuid,
) -> Result<CycleResultSummary, AppError> {
    let rows =
        fetch_proposals_for_cycle(db, &cycle.locale_slug, cycle.summary.id, Some(board_code))
            .await?;
    let solution_target_issue_id = if board_code == "solution" {
        load_solution_board_target_issue_id(db, cycle).await?
    } else {
        None
    };
    let proposals = if board_code == "solution" {
        filter_solution_proposals_for_target(map_proposal_rows(rows)?, solution_target_issue_id)
    } else {
        map_proposal_rows(rows)?
    };
    let candidates = build_cycle_outcome_candidates(proposals);
    let winner_proposal_id = candidates
        .iter()
        .find(|candidate| candidate.rank == Some(1))
        .map(|candidate| candidate.proposal.id);
    let ranked_candidate_count = candidates
        .iter()
        .filter(|candidate| candidate.rank.is_some())
        .count();
    let result_status = if board_code == "solution" && solution_target_issue_id.is_none() {
        "no_solution_target"
    } else if winner_proposal_id.is_some() {
        "resolved"
    } else {
        "no_ranked_winner"
    };

    let execution_record_id = if board_code == "solution" {
        match winner_proposal_id {
            Some(solution_proposal_id) => {
                let execution_record = create_execution_record_from_solution(
                    db,
                    &cycle.locale_slug,
                    moderator_user_id,
                    solution_proposal_id,
                    true,
                )
                .await?;
                Some(execution_record.summary.id)
            }
            None => None,
        }
    } else {
        None
    };

    let already_resolved = cycle_result_exists(db, cycle.summary.id, board_code).await?;
    let result_snapshot = json!({
        "cycle_id": cycle.summary.id,
        "locale_id": cycle.locale_id,
        "cycle_number": cycle.summary.cycle_number,
        "board_code": board_code,
        "winner_proposal_id": winner_proposal_id,
        "result_status": result_status,
        "solution_target_issue_proposal_id": solution_target_issue_id,
        "candidate_count": candidates.len(),
        "ranked_candidate_count": ranked_candidate_count,
        "candidates": candidates
    });

    let result_id = sqlx::query(
        r#"
        INSERT INTO cycle_results (
            cycle_id,
            locale_id,
            board_code,
            winning_proposal_id,
            execution_record_id,
            resolved_by_moderator_user_id,
            result_status,
            result_snapshot,
            published_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        ON CONFLICT (cycle_id, board_code)
        DO UPDATE SET
            winning_proposal_id = EXCLUDED.winning_proposal_id,
            execution_record_id = EXCLUDED.execution_record_id,
            resolved_by_moderator_user_id = EXCLUDED.resolved_by_moderator_user_id,
            result_status = EXCLUDED.result_status,
            result_snapshot = EXCLUDED.result_snapshot,
            published_at = COALESCE(cycle_results.published_at, NOW()),
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(cycle.summary.id)
    .bind(cycle.locale_id)
    .bind(board_code)
    .bind(winner_proposal_id)
    .bind(execution_record_id)
    .bind(moderator_user_id)
    .bind(result_status)
    .bind(result_snapshot)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error resolving cycle result: {}", err);
        AppError::Internal("Failed to resolve cycle result.".to_string())
    })?
    .try_get("id")
    .map_err(internal_db_err)?;

    let result = load_cycle_result_by_id(db, &cycle.locale_slug, result_id).await?;

    if !already_resolved {
        if let Some(winner_proposal_id) = winner_proposal_id {
            insert_moderator_action(
                db,
                "cycle_result_resolved",
                winner_proposal_id,
                None,
                moderator_user_id,
                Some("cycle_close"),
                None,
                None,
                json!({
                    "cycle_result_id": result.id,
                    "cycle_id": cycle.summary.id,
                    "cycle_number": cycle.summary.cycle_number,
                    "board_code": board_code,
                    "execution_record_id": execution_record_id,
                    "result_status": result_status
                }),
            )
            .await?;
        }
    }

    Ok(result)
}

async fn cycle_result_exists(
    db: &sqlx::PgPool,
    cycle_id: Uuid,
    board_code: &str,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM cycle_results
            WHERE cycle_id = $1
              AND board_code = $2
        ) AS exists_flag
        "#,
    )
    .bind(cycle_id)
    .bind(board_code)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking cycle result existence: {}", err);
        AppError::Internal("Failed to resolve cycle result.".to_string())
    })?;

    row.try_get("exists_flag").map_err(internal_db_err)
}

async fn load_cycle_result_by_id(
    db: &sqlx::PgPool,
    locale_slug: &str,
    result_id: Uuid,
) -> Result<CycleResultSummary, AppError> {
    let mut results =
        load_cycle_results_by_filter(db, locale_slug, Some(result_id), None, false).await?;
    results
        .pop()
        .ok_or_else(|| AppError::Internal("Resolved cycle result could not be loaded.".to_string()))
}

async fn load_cycle_results(
    db: &sqlx::PgPool,
    locale_slug: &str,
    cycle_id: Option<Uuid>,
    only_published: bool,
) -> Result<Vec<CycleResultSummary>, AppError> {
    load_cycle_results_by_filter(db, locale_slug, None, cycle_id, only_published).await
}

async fn load_cycle_results_by_filter(
    db: &sqlx::PgPool,
    locale_slug: &str,
    result_id: Option<Uuid>,
    cycle_id: Option<Uuid>,
    only_published: bool,
) -> Result<Vec<CycleResultSummary>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            cr.id,
            cr.cycle_id,
            c.cycle_number,
            cr.board_code,
            cr.result_status,
            cr.winning_proposal_id,
            cr.execution_record_id,
            cr.result_snapshot,
            cr.published_at,
            cr.created_at,
            cr.updated_at,
            wp.id AS winner_id,
            wb.code AS winner_board_code,
            wp.title AS winner_title,
            wp.primary_state AS winner_primary_state,
            wp.author_user_id AS winner_author_user_id,
            wp.parent_issue_proposal_id AS winner_parent_issue_proposal_id,
            wp.merged_into_proposal_id AS winner_merged_into_proposal_id,
            wp.archived_reason AS winner_archived_reason,
            wp.moderation_note AS winner_moderation_note,
            wp.support_count AS winner_support_count,
            wp.not_a_fit_count AS winner_not_a_fit_count,
            wp.unclear_count AS winner_unclear_count,
            wp.unsafe_count AS winner_unsafe_count,
            wp.merge_count AS winner_merge_count,
            wp.problem_description AS winner_problem_description,
            wp.affected_scope AS winner_affected_scope,
            wp.why_it_matters AS winner_why_it_matters,
            wp.action_description AS winner_action_description,
            wp.required_resource_categories AS winner_required_resource_categories,
            wp.completion_criteria AS winner_completion_criteria,
            wp.execution_tracking_entries AS winner_execution_tracking_entries,
            wp.created_at AS winner_created_at
        FROM cycle_results cr
        JOIN cycles c ON c.id = cr.cycle_id
        JOIN locales l ON l.id = cr.locale_id
        LEFT JOIN proposals wp ON wp.id = cr.winning_proposal_id
        LEFT JOIN boards wb ON wb.id = wp.board_id
        WHERE l.slug = $1
          AND ($2::uuid IS NULL OR cr.id = $2)
          AND ($3::uuid IS NULL OR cr.cycle_id = $3)
          AND ($4::boolean = FALSE OR cr.published_at IS NOT NULL)
        ORDER BY c.cycle_number DESC, cr.board_code ASC
        "#,
    )
    .bind(locale_slug)
    .bind(result_id)
    .bind(cycle_id)
    .bind(only_published)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!("database error loading cycle results: {}", err);
        AppError::Internal("Failed to load cycle results.".to_string())
    })?;

    rows.into_iter()
        .map(map_cycle_result_row)
        .collect::<Result<Vec<_>, AppError>>()
}

async fn fetch_archived_proposals(
    db: &sqlx::PgPool,
    locale_slug: &str,
) -> Result<Vec<sqlx::postgres::PgRow>, AppError> {
    sqlx::query(
        r#"
        SELECT
            p.id,
            b.code AS board_code,
            p.title,
            p.primary_state,
            p.author_user_id,
            p.parent_issue_proposal_id,
            p.merged_into_proposal_id,
            p.archived_reason,
            p.moderation_note,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.problem_description,
            p.affected_scope,
            p.why_it_matters,
            p.action_description,
            p.required_resource_categories,
            p.completion_criteria,
            p.execution_tracking_entries,
            p.created_at
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE l.slug = $1
          AND p.primary_state = 'archived'
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(locale_slug)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!("database error fetching archived proposals: {}", err);
        AppError::Internal("Failed to load archived proposals.".to_string())
    })
}

async fn fetch_reviewable_proposals_for_user(
    db: &sqlx::PgPool,
    locale_slug: &str,
    user_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<Vec<sqlx::postgres::PgRow>, AppError> {
    sqlx::query(
        r#"
        SELECT
            p.id,
            b.code AS board_code,
            p.title,
            p.primary_state,
            p.author_user_id,
            p.parent_issue_proposal_id,
            p.merged_into_proposal_id,
            p.archived_reason,
            p.moderation_note,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            p.problem_description,
            p.affected_scope,
            p.why_it_matters,
            p.action_description,
            p.required_resource_categories,
            p.completion_criteria,
            p.execution_tracking_entries,
            p.created_at,
            (
                SELECT COUNT(*)::bigint
                FROM review_actions all_ra
                WHERE all_ra.proposal_id = p.id
                  AND all_ra.cycle_id = p.cycle_id
            ) AS review_action_count,
            (
                SELECT COALESCE(AVG(review_counts.review_count), 0)::float8
                FROM (
                    SELECT COUNT(all_ra2.id)::numeric AS review_count
                    FROM proposals p2
                    JOIN boards b2 ON b2.id = p2.board_id
                    LEFT JOIN review_actions all_ra2
                        ON all_ra2.proposal_id = p2.id
                       AND all_ra2.cycle_id = p2.cycle_id
                    WHERE p2.cycle_id = p.cycle_id
                      AND b2.code = b.code
                      AND p2.primary_state = 'active'
                    GROUP BY p2.id
                ) review_counts
            ) AS cycle_average_review_action_count
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        LEFT JOIN review_actions ra
            ON ra.proposal_id = p.id
           AND ra.user_id = $1
           AND ra.cycle_id = p.cycle_id
        LEFT JOIN proposal_sentiment_votes sv
            ON sv.proposal_id = p.id
           AND sv.user_id = $1
        LEFT JOIN proposal_merge_votes mv
            ON mv.proposal_id = p.id
           AND mv.user_id = $1
        WHERE l.slug = $3
          AND c.is_active = TRUE
          AND p.primary_state = 'active'
          AND p.author_user_id <> $1
          AND b.code IN ('issue', 'solution')
          AND ($2::text IS NULL OR b.code = $2)
          AND ra.id IS NULL
          AND sv.id IS NULL
          AND mv.id IS NULL
          AND NOT EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
              )
          AND NOT EXISTS (
                SELECT 1
                FROM reconsideration_windows rw
                WHERE rw.proposal_id = p.id
                  AND rw.status = 'open'
                  AND rw.ends_at <= NOW()
                  AND (
                    p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
              )
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(user_id)
    .bind(board_code_filter)
    .bind(locale_slug)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!("database error fetching user reviewable proposals: {}", err);
        AppError::Internal("Failed to load review pool.".to_string())
    })
}

async fn load_merge_relationships(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    include_pair_thresholds: bool,
) -> Result<ProposalMergeRelationships, AppError> {
    let outgoing_rows = sqlx::query(
        r#"
        SELECT
            r.source_proposal_id,
            r.target_proposal_id,
            sp.title AS source_title,
            tp.title AS target_title,
            r.status AS relationship_status,
            r.created_at AS relationship_created_at,
            (
                SELECT COUNT(*)::int
                FROM proposal_merge_votes mv
                WHERE mv.proposal_id = r.source_proposal_id
                  AND mv.target_proposal_id = r.target_proposal_id
            ) AS source_to_target_merge_count,
            (
                SELECT COUNT(*)::int
                FROM proposal_merge_votes mv
                WHERE mv.proposal_id = r.target_proposal_id
                  AND mv.target_proposal_id = r.source_proposal_id
            ) AS target_to_source_merge_count,
            (
                sp.support_count + sp.not_a_fit_count + sp.unclear_count
                + sp.unsafe_count + sp.merge_count
            ) AS source_total_count,
            (
                tp.support_count + tp.not_a_fit_count + tp.unclear_count
                + tp.unsafe_count + tp.merge_count
            ) AS target_total_count,
            n.author_user_id,
            n.difference_type,
            n.note_text,
            n.created_at AS note_created_at,
            n.updated_at AS note_updated_at
        FROM proposal_merge_relationships r
        JOIN proposals sp ON sp.id = r.source_proposal_id
        JOIN proposals tp ON tp.id = r.target_proposal_id
        LEFT JOIN merge_distinction_notes n
            ON n.source_proposal_id = r.source_proposal_id
           AND n.target_proposal_id = r.target_proposal_id
        WHERE r.source_proposal_id = $1
          AND r.status = 'active'
        ORDER BY r.updated_at DESC
        "#,
    )
    .bind(proposal_id)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading outgoing merge relationships: {}",
            err
        );
        AppError::Internal("Failed to load merge relationships.".to_string())
    })?;

    let incoming_rows = sqlx::query(
        r#"
        SELECT
            r.source_proposal_id,
            r.target_proposal_id,
            sp.title AS source_title,
            tp.title AS target_title,
            r.status AS relationship_status,
            r.created_at AS relationship_created_at,
            (
                SELECT COUNT(*)::int
                FROM proposal_merge_votes mv
                WHERE mv.proposal_id = r.source_proposal_id
                  AND mv.target_proposal_id = r.target_proposal_id
            ) AS source_to_target_merge_count,
            (
                SELECT COUNT(*)::int
                FROM proposal_merge_votes mv
                WHERE mv.proposal_id = r.target_proposal_id
                  AND mv.target_proposal_id = r.source_proposal_id
            ) AS target_to_source_merge_count,
            (
                sp.support_count + sp.not_a_fit_count + sp.unclear_count
                + sp.unsafe_count + sp.merge_count
            ) AS source_total_count,
            (
                tp.support_count + tp.not_a_fit_count + tp.unclear_count
                + tp.unsafe_count + tp.merge_count
            ) AS target_total_count,
            n.author_user_id,
            n.difference_type,
            n.note_text,
            n.created_at AS note_created_at,
            n.updated_at AS note_updated_at
        FROM proposal_merge_relationships r
        JOIN proposals sp ON sp.id = r.source_proposal_id
        JOIN proposals tp ON tp.id = r.target_proposal_id
        LEFT JOIN merge_distinction_notes n
            ON n.source_proposal_id = r.source_proposal_id
           AND n.target_proposal_id = r.target_proposal_id
        WHERE r.target_proposal_id = $1
          AND r.status = 'active'
        ORDER BY r.updated_at DESC
        "#,
    )
    .bind(proposal_id)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading incoming merge relationships: {}",
            err
        );
        AppError::Internal("Failed to load merge relationships.".to_string())
    })?;

    Ok(ProposalMergeRelationships {
        outgoing: map_merge_relationship_rows(outgoing_rows, include_pair_thresholds)?,
        incoming: map_merge_relationship_rows(incoming_rows, include_pair_thresholds)?,
    })
}

fn map_merge_relationship_rows(
    rows: Vec<sqlx::postgres::PgRow>,
    include_pair_thresholds: bool,
) -> Result<Vec<ProposalMergeRelationship>, AppError> {
    rows.into_iter()
        .map(|row| {
            let source_to_target_merge_count: i32 = row
                .try_get("source_to_target_merge_count")
                .map_err(internal_db_err)?;
            let target_to_source_merge_count: i32 = row
                .try_get("target_to_source_merge_count")
                .map_err(internal_db_err)?;
            let source_total_count: i32 =
                row.try_get("source_total_count").map_err(internal_db_err)?;
            let target_total_count: i32 =
                row.try_get("target_total_count").map_err(internal_db_err)?;

            Ok(ProposalMergeRelationship {
                source_proposal_id: row.try_get("source_proposal_id").map_err(internal_db_err)?,
                target_proposal_id: row.try_get("target_proposal_id").map_err(internal_db_err)?,
                source_title: row.try_get("source_title").map_err(internal_db_err)?,
                target_title: row.try_get("target_title").map_err(internal_db_err)?,
                relationship_status: row
                    .try_get("relationship_status")
                    .map_err(internal_db_err)?,
                relationship_created_at: row
                    .try_get("relationship_created_at")
                    .map_err(internal_db_err)?,
                source_to_target_high_merge_watch: include_pair_thresholds.then_some(
                    high_merge_watch_for_pair(source_total_count, source_to_target_merge_count),
                ),
                target_to_source_high_merge_watch: include_pair_thresholds.then_some(
                    high_merge_watch_for_pair(target_total_count, target_to_source_merge_count),
                ),
                note: map_relationship_note(&row)?,
            })
        })
        .collect()
}

fn map_relationship_note(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<ProposalMergeRelationshipNote>, AppError> {
    let note_text: Option<String> = row.try_get("note_text").map_err(internal_db_err)?;

    let Some(note_text) = note_text else {
        return Ok(None);
    };

    let created_at: DateTime<Utc> = row.try_get("note_created_at").map_err(internal_db_err)?;
    let updated_at: DateTime<Utc> = row.try_get("note_updated_at").map_err(internal_db_err)?;

    Ok(Some(ProposalMergeRelationshipNote {
        difference_type: row.try_get("difference_type").map_err(internal_db_err)?,
        note_text,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    }))
}

fn map_proposal_rows(rows: Vec<sqlx::postgres::PgRow>) -> Result<Vec<ProposalSummary>, AppError> {
    rows.into_iter()
        .map(map_one_proposal_row)
        .collect::<Result<Vec<_>, AppError>>()
}

fn map_cycle_result_row(row: sqlx::postgres::PgRow) -> Result<CycleResultSummary, AppError> {
    Ok(CycleResultSummary {
        id: row.try_get("id").map_err(internal_db_err)?,
        cycle_id: row.try_get("cycle_id").map_err(internal_db_err)?,
        cycle_number: row.try_get("cycle_number").map_err(internal_db_err)?,
        board_code: row.try_get("board_code").map_err(internal_db_err)?,
        result_status: row.try_get("result_status").map_err(internal_db_err)?,
        winning_proposal_id: row
            .try_get("winning_proposal_id")
            .map_err(internal_db_err)?,
        execution_record_id: row
            .try_get("execution_record_id")
            .map_err(internal_db_err)?,
        result_snapshot: row.try_get("result_snapshot").map_err(internal_db_err)?,
        published_at: row.try_get("published_at").map_err(internal_db_err)?,
        created_at: row.try_get("created_at").map_err(internal_db_err)?,
        updated_at: row.try_get("updated_at").map_err(internal_db_err)?,
        winning_proposal: map_winning_proposal_from_result_row(&row)?,
    })
}

fn map_merge_proposal_row(row: sqlx::postgres::PgRow) -> Result<MergeProposal, AppError> {
    Ok(MergeProposal {
        id: row.try_get("id").map_err(internal_db_err)?,
        title: row.try_get("title").map_err(internal_db_err)?,
        board_code: row.try_get("board_code").map_err(internal_db_err)?,
        cycle_id: row.try_get("cycle_id").map_err(internal_db_err)?,
        locale_id: row.try_get("locale_id").map_err(internal_db_err)?,
        primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
        frozen_for_review: row.try_get("frozen_for_review").map_err(internal_db_err)?,
        reconsideration_window_open: row
            .try_get("reconsideration_window_open")
            .map_err(internal_db_err)?,
        counts: proposal_counts_from_row(&row)?,
    })
}

async fn load_pair_merge_threshold(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_proposal: &MergeProposal,
    target_proposal: &MergeProposal,
) -> Result<PairMergeThreshold, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            (
                SELECT COUNT(*)::int
                FROM proposal_merge_votes
                WHERE proposal_id = $1
                  AND target_proposal_id = $2
            ) AS source_to_target_merge_count,
            (
                SELECT COUNT(*)::int
                FROM proposal_merge_votes
                WHERE proposal_id = $2
                  AND target_proposal_id = $1
            ) AS target_to_source_merge_count,
            EXISTS (
                SELECT 1
                FROM proposal_merge_relationships
                WHERE status = 'active'
                  AND (
                    (source_proposal_id = $1 AND target_proposal_id = $2)
                    OR (source_proposal_id = $2 AND target_proposal_id = $1)
                  )
            ) AS relationship_exists
        "#,
    )
    .bind(source_proposal.id)
    .bind(target_proposal.id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| {
        error!(
            "database error checking pair-specific merge threshold: {}",
            err
        );
        AppError::Internal("Failed to execute merge.".to_string())
    })?;

    let source_to_target_merge_count: i32 = row
        .try_get("source_to_target_merge_count")
        .map_err(internal_db_err)?;
    let target_to_source_merge_count: i32 = row
        .try_get("target_to_source_merge_count")
        .map_err(internal_db_err)?;
    let relationship_exists: bool = row
        .try_get("relationship_exists")
        .map_err(internal_db_err)?;

    Ok(PairMergeThreshold {
        relationship_exists,
        source_to_target_merge_count,
        target_to_source_merge_count,
        source_to_target_high_merge_watch: source_proposal
            .counts
            .high_merge_watch_for_target(source_to_target_merge_count),
        target_to_source_high_merge_watch: target_proposal
            .counts
            .high_merge_watch_for_target(target_to_source_merge_count),
    })
}

fn map_winning_proposal_from_result_row(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<ProposalSummary>, AppError> {
    let id: Option<Uuid> = row.try_get("winner_id").map_err(internal_db_err)?;
    let Some(id) = id else {
        return Ok(None);
    };

    let required = |field: &'static str| {
        AppError::Internal(format!("Cycle result is missing winner field: {field}."))
    };

    Ok(Some(ProposalSummary {
        id,
        board_code: row
            .try_get::<Option<String>, _>("winner_board_code")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("board_code"))?,
        title: row
            .try_get::<Option<String>, _>("winner_title")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("title"))?,
        primary_state: row
            .try_get::<Option<String>, _>("winner_primary_state")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("primary_state"))?,
        author_user_id: row
            .try_get::<Option<Uuid>, _>("winner_author_user_id")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("author_user_id"))?,
        parent_issue_proposal_id: row
            .try_get("winner_parent_issue_proposal_id")
            .map_err(internal_db_err)?,
        merged_into_proposal_id: row
            .try_get("winner_merged_into_proposal_id")
            .map_err(internal_db_err)?,
        archived_reason: row
            .try_get("winner_archived_reason")
            .map_err(internal_db_err)?,
        moderation_note: row
            .try_get("winner_moderation_note")
            .map_err(internal_db_err)?,
        support_count: row
            .try_get::<Option<i32>, _>("winner_support_count")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("support_count"))?,
        not_a_fit_count: row
            .try_get::<Option<i32>, _>("winner_not_a_fit_count")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("not_a_fit_count"))?,
        unclear_count: row
            .try_get::<Option<i32>, _>("winner_unclear_count")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("unclear_count"))?,
        unsafe_count: row
            .try_get::<Option<i32>, _>("winner_unsafe_count")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("unsafe_count"))?,
        merge_count: row
            .try_get::<Option<i32>, _>("winner_merge_count")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("merge_count"))?,
        problem_description: row
            .try_get("winner_problem_description")
            .map_err(internal_db_err)?,
        affected_scope: row
            .try_get("winner_affected_scope")
            .map_err(internal_db_err)?,
        why_it_matters: row
            .try_get("winner_why_it_matters")
            .map_err(internal_db_err)?,
        action_description: row
            .try_get("winner_action_description")
            .map_err(internal_db_err)?,
        required_resource_categories: row
            .try_get("winner_required_resource_categories")
            .map_err(internal_db_err)?,
        completion_criteria: row
            .try_get("winner_completion_criteria")
            .map_err(internal_db_err)?,
        execution_tracking_entries: row
            .try_get("winner_execution_tracking_entries")
            .map_err(internal_db_err)?,
        created_at: row
            .try_get::<Option<DateTime<Utc>>, _>("winner_created_at")
            .map_err(internal_db_err)?
            .ok_or_else(|| required("created_at"))?,
        high_moderation_watch_started_at: None,
        review_action_count: 0,
        cycle_average_review_action_count: 0.0,
    }))
}

fn build_cycle_outcome_candidates(proposals: Vec<ProposalSummary>) -> Vec<CycleOutcomeCandidate> {
    let mut candidates = proposals
        .into_iter()
        .map(|proposal| {
            let counts = proposal_counts_from_summary(&proposal);
            let non_merge_count = counts.non_merge_count();
            let negative_count = counts.negative_count();
            let total_count = counts.total_count();
            let support_ratio = if non_merge_count > 0 {
                Some(counts.support as f64 / non_merge_count as f64)
            } else {
                None
            };
            let unsafe_fraction = if non_merge_count > 0 {
                Some(counts.unsafe_count as f64 / non_merge_count as f64)
            } else {
                None
            };

            CycleOutcomeCandidate {
                proposal,
                classification: if non_merge_count >= 12 {
                    "ranked".to_string()
                } else {
                    "emerging".to_string()
                },
                rank: None,
                support_ratio,
                unsafe_fraction,
                negative_count,
                non_merge_count,
                total_count,
            }
        })
        .collect::<Vec<_>>();

    candidates.sort_by(compare_cycle_outcome_candidates);

    let mut rank = 1;
    for candidate in &mut candidates {
        if candidate.classification == "ranked" {
            candidate.rank = Some(rank);
            rank += 1;
        }
    }

    candidates
}

fn compare_cycle_outcome_candidates(
    a: &CycleOutcomeCandidate,
    b: &CycleOutcomeCandidate,
) -> std::cmp::Ordering {
    let a_ranked = a.classification == "ranked";
    let b_ranked = b.classification == "ranked";

    match (a_ranked, b_ranked) {
        (true, false) => return std::cmp::Ordering::Less,
        (false, true) => return std::cmp::Ordering::Greater,
        _ => {}
    }

    if a_ranked && b_ranked {
        return b
            .support_ratio
            .unwrap_or(0.0)
            .partial_cmp(&a.support_ratio.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.non_merge_count.cmp(&a.non_merge_count))
            .then_with(|| {
                a.unsafe_fraction
                    .unwrap_or(0.0)
                    .partial_cmp(&b.unsafe_fraction.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.proposal.created_at.cmp(&b.proposal.created_at));
    }

    b.non_merge_count
        .cmp(&a.non_merge_count)
        .then_with(|| b.total_count.cmp(&a.total_count))
        .then_with(|| a.proposal.created_at.cmp(&b.proposal.created_at))
}

fn map_one_proposal_row(row: sqlx::postgres::PgRow) -> Result<ProposalSummary, AppError> {
    Ok(ProposalSummary {
        id: row.try_get("id").map_err(internal_db_err)?,
        board_code: row.try_get("board_code").map_err(internal_db_err)?,
        title: row.try_get("title").map_err(internal_db_err)?,
        primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
        author_user_id: row.try_get("author_user_id").map_err(internal_db_err)?,
        parent_issue_proposal_id: row
            .try_get("parent_issue_proposal_id")
            .map_err(internal_db_err)?,
        merged_into_proposal_id: row
            .try_get("merged_into_proposal_id")
            .map_err(internal_db_err)?,
        archived_reason: row.try_get("archived_reason").map_err(internal_db_err)?,
        moderation_note: row.try_get("moderation_note").map_err(internal_db_err)?,
        support_count: row.try_get("support_count").map_err(internal_db_err)?,
        not_a_fit_count: row.try_get("not_a_fit_count").map_err(internal_db_err)?,
        unclear_count: row.try_get("unclear_count").map_err(internal_db_err)?,
        unsafe_count: row.try_get("unsafe_count").map_err(internal_db_err)?,
        merge_count: row.try_get("merge_count").map_err(internal_db_err)?,
        problem_description: row
            .try_get("problem_description")
            .map_err(internal_db_err)?,
        affected_scope: row.try_get("affected_scope").map_err(internal_db_err)?,
        why_it_matters: row.try_get("why_it_matters").map_err(internal_db_err)?,
        action_description: row.try_get("action_description").map_err(internal_db_err)?,
        required_resource_categories: row
            .try_get("required_resource_categories")
            .map_err(internal_db_err)?,
        completion_criteria: row
            .try_get("completion_criteria")
            .map_err(internal_db_err)?,
        execution_tracking_entries: row
            .try_get("execution_tracking_entries")
            .map_err(internal_db_err)?,
        created_at: row.try_get("created_at").map_err(internal_db_err)?,
        high_moderation_watch_started_at: row
            .try_get("high_moderation_watch_started_at")
            .unwrap_or(None),
        review_action_count: row.try_get("review_action_count").unwrap_or(0),
        cycle_average_review_action_count: row
            .try_get("cycle_average_review_action_count")
            .unwrap_or(0.0),
    })
}

fn order_review_feed_proposals(proposals: Vec<ProposalSummary>) -> Vec<ProposalSummary> {
    let mut remaining = build_review_candidates(proposals, false);
    let mut ordered = Vec::new();

    while !remaining.is_empty() {
        let selected = select_review_pool(remaining.clone(), STANDARD_REQUIRED_REVIEW_COUNT);
        if selected.is_empty() {
            break;
        }

        let selected_ids: Vec<Uuid> = selected.iter().map(|item| item.proposal.id).collect();
        ordered.extend(selected.into_iter().map(|item| item.proposal));
        remaining.retain(|candidate| !selected_ids.contains(&candidate.proposal.id));
    }

    ordered
}

fn build_review_candidates(
    proposals: Vec<ProposalSummary>,
    exclude_unreviewable: bool,
) -> Vec<ReviewCandidate> {
    let candidates = proposals.into_iter().map(|proposal| {
        let support = proposal.support_count;
        let negative = proposal.not_a_fit_count + proposal.unclear_count + proposal.unsafe_count;
        let sentiment_total = support + negative;
        let total_interactions = sentiment_total + proposal.merge_count;
        let like_floor = support.max(1);
        let dislike_ratio = negative as f64 / like_floor as f64;
        let merge_fraction = if total_interactions > 0 {
            proposal.merge_count as f64 / total_interactions as f64
        } else {
            0.0
        };
        let support_ratio = if sentiment_total > 0 {
            support as f64 / sentiment_total as f64
        } else {
            0.0
        };

        ReviewCandidate {
            review_action_count: proposal.review_action_count,
            cycle_average_review_action_count: proposal.cycle_average_review_action_count,
            proposal,
            total_interactions,
            sentiment_total,
            merge_fraction,
            dislike_ratio,
            support_ratio,
        }
    });

    if exclude_unreviewable {
        candidates
            .filter(|candidate| !candidate_is_excluded(candidate))
            .collect()
    } else {
        candidates.collect()
    }
}

fn candidate_is_excluded(candidate: &ReviewCandidate) -> bool {
    candidate.dislike_ratio > 8.0
        || candidate.proposal.unsafe_count >= 8
        || fraction_at_least(
            candidate.proposal.unsafe_count,
            candidate.total_interactions,
            0.50,
        )
}

fn select_review_pool(candidates: Vec<ReviewCandidate>, limit: usize) -> Vec<ReviewPoolProposal> {
    let mut selected: Vec<ReviewPoolProposal> = Vec::new();
    let mut selected_ids: Vec<Uuid> = Vec::new();

    let slot_order = [
        ReviewBucket::LowExposure,
        ReviewBucket::ContestedUnderReviewed,
        ReviewBucket::MergeHeavy,
        ReviewBucket::LowRatedSalvageable,
    ];

    for bucket in slot_order {
        if selected.len() >= limit {
            break;
        }

        if let Some(item) = pick_best_candidate(&candidates, &selected_ids, bucket) {
            selected_ids.push(item.proposal.id);
            selected.push(ReviewPoolProposal {
                proposal: item.proposal,
            });
        }
    }

    while selected.len() < limit {
        let Some(item) = pick_best_candidate(&candidates, &selected_ids, ReviewBucket::Fallback)
        else {
            break;
        };

        selected_ids.push(item.proposal.id);
        selected.push(ReviewPoolProposal {
            proposal: item.proposal,
        });
    }

    selected.truncate(limit);
    selected
}

fn pick_best_candidate(
    candidates: &[ReviewCandidate],
    selected_ids: &[Uuid],
    bucket: ReviewBucket,
) -> Option<ReviewCandidate> {
    let mut bucket_items: Vec<ReviewCandidate> = candidates
        .iter()
        .filter(|c| !selected_ids.contains(&c.proposal.id))
        .filter(|c| assigned_review_bucket(c) == bucket)
        .cloned()
        .collect();

    if bucket_items.is_empty() {
        return None;
    }

    sort_bucket(&mut bucket_items, bucket);
    bucket_items.into_iter().next()
}

fn assigned_review_bucket(candidate: &ReviewCandidate) -> ReviewBucket {
    if candidate_matches_bucket(candidate, ReviewBucket::LowExposure) {
        ReviewBucket::LowExposure
    } else if candidate_matches_bucket(candidate, ReviewBucket::ContestedUnderReviewed) {
        ReviewBucket::ContestedUnderReviewed
    } else if candidate_matches_bucket(candidate, ReviewBucket::MergeHeavy) {
        ReviewBucket::MergeHeavy
    } else if candidate_matches_bucket(candidate, ReviewBucket::LowRatedSalvageable) {
        ReviewBucket::LowRatedSalvageable
    } else {
        ReviewBucket::Fallback
    }
}

fn candidate_matches_bucket(candidate: &ReviewCandidate, bucket: ReviewBucket) -> bool {
    match bucket {
        ReviewBucket::LowRatedSalvageable => {
            candidate.sentiment_total >= 3 && candidate.dislike_ratio <= 4.0
        }
        ReviewBucket::ContestedUnderReviewed => {
            (6..=20).contains(&candidate.sentiment_total)
                && candidate.support_ratio >= 0.40
                && candidate.support_ratio <= 0.60
        }
        ReviewBucket::MergeHeavy => {
            candidate.total_interactions >= 10 && candidate.merge_fraction >= 0.20
        }
        ReviewBucket::LowExposure => {
            candidate.total_interactions < 12
                && (candidate.review_action_count as f64)
                    <= candidate.cycle_average_review_action_count.max(1.0).ceil()
        }
        ReviewBucket::Fallback => true,
    }
}

fn sort_bucket(items: &mut [ReviewCandidate], bucket: ReviewBucket) {
    match bucket {
        ReviewBucket::LowRatedSalvageable => {
            items.sort_by(|a, b| {
                a.dislike_ratio
                    .partial_cmp(&b.dislike_ratio)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.sentiment_total.cmp(&b.sentiment_total))
                    .then_with(|| a.proposal.created_at.cmp(&b.proposal.created_at))
            });
        }
        ReviewBucket::ContestedUnderReviewed => {
            items.sort_by(|a, b| {
                a.sentiment_total
                    .cmp(&b.sentiment_total)
                    .then_with(|| a.proposal.created_at.cmp(&b.proposal.created_at))
            });
        }
        ReviewBucket::MergeHeavy => {
            items.sort_by(|a, b| {
                b.merge_fraction
                    .partial_cmp(&a.merge_fraction)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.total_interactions.cmp(&b.total_interactions))
                    .then_with(|| a.proposal.created_at.cmp(&b.proposal.created_at))
            });
        }
        ReviewBucket::LowExposure | ReviewBucket::Fallback => {
            items.sort_by(|a, b| {
                let a_total_exposure = a.total_interactions as i64 + a.review_action_count;
                let b_total_exposure = b.total_interactions as i64 + b.review_action_count;

                a_total_exposure
                    .cmp(&b_total_exposure)
                    .then_with(|| a.sentiment_total.cmp(&b.sentiment_total))
                    .then_with(|| a.proposal.created_at.cmp(&b.proposal.created_at))
            });
        }
    }
}

async fn load_moderator_actions(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
) -> Result<Vec<ModeratorActionSummary>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            ma.id,
            ma.action_type,
            ma.proposal_id,
            ma.related_proposal_id,
            rp.title AS related_proposal_title,
            ma.action_reason,
            ma.public_note,
            ma.created_at
        FROM moderator_actions ma
        LEFT JOIN proposals rp ON rp.id = ma.related_proposal_id
        WHERE ma.proposal_id = $1
           OR ma.related_proposal_id = $1
        ORDER BY ma.created_at DESC
        "#,
    )
    .bind(proposal_id)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!("database error loading moderator actions: {}", err);
        AppError::Internal("Failed to load moderation history.".to_string())
    })?;

    rows.into_iter()
        .map(|row| {
            Ok(ModeratorActionSummary {
                id: row.try_get("id").map_err(internal_db_err)?,
                action_type: row.try_get("action_type").map_err(internal_db_err)?,
                proposal_id: row.try_get("proposal_id").map_err(internal_db_err)?,
                related_proposal_id: row
                    .try_get("related_proposal_id")
                    .map_err(internal_db_err)?,
                related_proposal_title: row
                    .try_get("related_proposal_title")
                    .map_err(internal_db_err)?,
                action_reason: row.try_get("action_reason").map_err(internal_db_err)?,
                public_note: row.try_get("public_note").map_err(internal_db_err)?,
                created_at: row.try_get("created_at").map_err(internal_db_err)?,
            })
        })
        .collect()
}

async fn insert_moderator_action(
    db: &sqlx::PgPool,
    action_type: &str,
    proposal_id: Uuid,
    related_proposal_id: Option<Uuid>,
    moderator_user_id: Uuid,
    action_reason: Option<&str>,
    public_note: Option<&str>,
    internal_note: Option<&str>,
    state_snapshot: Value,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO moderator_actions (
            action_type,
            proposal_id,
            related_proposal_id,
            moderator_user_id,
            action_reason,
            public_note,
            internal_note,
            state_snapshot
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(action_type)
    .bind(proposal_id)
    .bind(related_proposal_id)
    .bind(moderator_user_id)
    .bind(action_reason)
    .bind(public_note)
    .bind(internal_note)
    .bind(state_snapshot)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting moderator action: {}", err);
        AppError::Internal("Failed to log moderator action.".to_string())
    })?;

    Ok(())
}

async fn merge_distinction_note_exists(
    db: &sqlx::PgPool,
    source_proposal_id: Uuid,
    target_proposal_id: Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM merge_distinction_notes
            WHERE (
                source_proposal_id = $1
                AND target_proposal_id = $2
            )
               OR (
                source_proposal_id = $2
                AND target_proposal_id = $1
            )
        ) AS exists_flag
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!(
            "database error checking distinction note existence: {}",
            err
        );
        AppError::Internal("Failed to execute merge.".to_string())
    })?;

    row.try_get("exists_flag").map_err(internal_db_err)
}

async fn proposal_has_active_watch_flag(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    flag_code: &str,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM proposal_watch_flags
            WHERE proposal_id = $1
              AND flag_code = $2
              AND cleared_at IS NULL
        ) AS exists_flag
        "#,
    )
    .bind(proposal_id)
    .bind(flag_code)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking proposal watch flag: {}", err);
        AppError::Internal("Failed to read proposal watch state.".to_string())
    })?;

    row.try_get("exists_flag").map_err(internal_db_err)
}

async fn insert_frozen_for_review_flag(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    moderator_user_id: Option<Uuid>,
    reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO proposal_watch_flags (
            proposal_id,
            flag_code,
            created_by_moderator_user_id,
            reason
        )
        VALUES ($1, 'frozen_for_review', $2, $3)
        "#,
    )
    .bind(proposal_id)
    .bind(moderator_user_id)
    .bind(reason)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting frozen watch flag: {}", err);
        AppError::Internal("Failed to freeze proposal.".to_string())
    })?;

    Ok(())
}

async fn clear_frozen_for_review_flag(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    moderator_user_id: Option<Uuid>,
    clearance_reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE proposal_watch_flags
        SET
            cleared_at = NOW(),
            cleared_by_moderator_user_id = $2,
            clearance_reason = $3
        WHERE proposal_id = $1
          AND flag_code = 'frozen_for_review'
          AND cleared_at IS NULL
        "#,
    )
    .bind(proposal_id)
    .bind(moderator_user_id)
    .bind(clearance_reason)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error clearing frozen watch flag: {}", err);
        AppError::Internal("Failed to clear frozen proposal state.".to_string())
    })?;

    Ok(())
}

async fn reconcile_sentiment_votes_for_merge(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_proposal_id: Uuid,
    target_proposal_id: Uuid,
) -> Result<SentimentVoteReconciliation, AppError> {
    let rows = sqlx::query(
        r#"
        INSERT INTO proposal_merge_vote_reconciliations (
            source_proposal_id,
            target_proposal_id,
            user_id,
            vote_kind,
            source_vote_value,
            target_existing_vote_value,
            outcome
        )
        SELECT
            $1,
            $2,
            sv.user_id,
            'sentiment',
            sv.vote_value,
            tv.vote_value,
            CASE
                WHEN tv.id IS NULL THEN 'transferred'
                WHEN tv.vote_value = sv.vote_value THEN 'discarded_same_target_vote'
                ELSE 'discarded_conflicting_target_vote'
            END
        FROM proposal_sentiment_votes sv
        LEFT JOIN proposal_sentiment_votes tv
            ON tv.proposal_id = $2
           AND tv.user_id = sv.user_id
        WHERE sv.proposal_id = $1
        RETURNING outcome
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|err| {
        error!("database error auditing merge vote reconciliation: {}", err);
        AppError::Internal("Failed to reconcile merge votes.".to_string())
    })?;

    let mut summary = SentimentVoteReconciliation {
        transferred: 0,
        discarded_same: 0,
        discarded_conflicting: 0,
    };

    for row in rows {
        let outcome: String = row.try_get("outcome").map_err(internal_db_err)?;
        match outcome.as_str() {
            "transferred" => summary.transferred += 1,
            "discarded_same_target_vote" => summary.discarded_same += 1,
            "discarded_conflicting_target_vote" => summary.discarded_conflicting += 1,
            _ => {}
        }
    }

    let transferred_rows = sqlx::query(
        r#"
        INSERT INTO proposal_sentiment_votes (
            proposal_id,
            user_id,
            vote_value,
            created_at,
            updated_at
        )
        SELECT
            $2,
            sv.user_id,
            sv.vote_value,
            sv.created_at,
            NOW()
        FROM proposal_sentiment_votes sv
        LEFT JOIN proposal_sentiment_votes tv
            ON tv.proposal_id = $2
           AND tv.user_id = sv.user_id
        WHERE sv.proposal_id = $1
          AND tv.id IS NULL
        ON CONFLICT (proposal_id, user_id)
        DO NOTHING
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .execute(&mut **tx)
    .await
    .map_err(|err| {
        error!("database error transferring merge sentiment votes: {}", err);
        AppError::Internal("Failed to transfer merge votes.".to_string())
    })?
    .rows_affected() as i64;

    summary.transferred = transferred_rows;

    sqlx::query(
        r#"
        DELETE FROM proposal_sentiment_votes
        WHERE proposal_id = $1
        "#,
    )
    .bind(source_proposal_id)
    .execute(&mut **tx)
    .await
    .map_err(|err| {
        error!("database error removing merged sentiment votes: {}", err);
        AppError::Internal("Failed to reconcile merge votes.".to_string())
    })?;

    Ok(summary)
}

async fn refresh_proposal_vote_counts_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    proposal_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        WITH counts AS (
            SELECT
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND user_id <> (SELECT author_user_id FROM proposals WHERE id = $1)
                      AND vote_value = 'support'
                ) AS support_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND user_id <> (SELECT author_user_id FROM proposals WHERE id = $1)
                      AND vote_value = 'not_a_fit'
                ) AS not_a_fit_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND user_id <> (SELECT author_user_id FROM proposals WHERE id = $1)
                      AND vote_value = 'unclear'
                ) AS unclear_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND user_id <> (SELECT author_user_id FROM proposals WHERE id = $1)
                      AND vote_value = 'unsafe'
                ) AS unsafe_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_merge_votes mv
                    WHERE mv.proposal_id = $1
                      AND mv.user_id <> (SELECT author_user_id FROM proposals WHERE id = $1)
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
    .execute(&mut **tx)
    .await
    .map_err(|err| {
        error!("database error refreshing proposal vote counts: {}", err);
        AppError::Internal("Failed to refresh vote counts.".to_string())
    })?;

    Ok(())
}

fn require_moderator(auth_user: &AuthUser) -> Result<(), AppError> {
    if auth_user.can_moderate() {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Moderator privileges are required for this action.".to_string(),
        ))
    }
}

fn proposal_counts_from_row(row: &sqlx::postgres::PgRow) -> Result<ProposalCounts, AppError> {
    Ok(ProposalCounts {
        support: row.try_get("support_count").map_err(internal_db_err)?,
        not_a_fit: row.try_get("not_a_fit_count").map_err(internal_db_err)?,
        unclear: row.try_get("unclear_count").map_err(internal_db_err)?,
        unsafe_count: row.try_get("unsafe_count").map_err(internal_db_err)?,
        merge_count: row.try_get("merge_count").map_err(internal_db_err)?,
    })
}

fn proposal_counts_from_summary(proposal: &ProposalSummary) -> ProposalCounts {
    ProposalCounts {
        support: proposal.support_count,
        not_a_fit: proposal.not_a_fit_count,
        unclear: proposal.unclear_count,
        unsafe_count: proposal.unsafe_count,
        merge_count: proposal.merge_count,
    }
}

fn high_merge_watch_for_pair(total_count: i32, target_merge_count: i32) -> bool {
    total_count >= 20 && fraction_at_least(target_merge_count, total_count, 0.35)
}

fn fraction_at_least(part: i32, total: i32, threshold: f64) -> bool {
    total > 0 && (part as f64 / total as f64) >= threshold
}

fn is_valid_archive_reason(value: &str) -> bool {
    matches!(
        value,
        "duplicate"
            | "unsafe_illegal_deceptive"
            | "spam_abuse"
            | "irrelevant"
            | "minimum_quality"
            | "superseded"
            | "moderation"
            | "manual_archive"
            | "not_a_fit"
    )
}

fn validate_title_quality(value: &str) -> Result<(), AppError> {
    validate_required_text(value, "Title", MAX_TITLE_CHARS)
}

fn require_submission_text(
    value: &Option<String>,
    field_name: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    let Some(value) = value.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty()) else {
        return Err(AppError::BadRequest(format!("{field_name} is required.")));
    };

    validate_required_text(value, field_name, max_chars)
}

fn validate_required_text(value: &str, field_name: &str, max_chars: usize) -> Result<(), AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(format!("{field_name} is required.")));
    }

    validate_text_max_chars(trimmed, field_name, max_chars)?;

    let lowered = trimmed.to_ascii_lowercase();
    let compact = lowered
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect::<String>();
    let obvious_placeholder = matches!(
        compact.as_str(),
        "test"
            | "testing"
            | "asdf"
            | "qwerty"
            | "todo"
            | "tbd"
            | "na"
            | "none"
            | "null"
            | "placeholder"
            | "loremipsum"
    ) || lowered.contains("lorem ipsum");

    if obvious_placeholder {
        return Err(AppError::BadRequest(format!(
            "{field_name} looks like placeholder text."
        )));
    }

    Ok(())
}

fn validate_text_max_chars(
    value: &str,
    field_name: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    if value.chars().count() > max_chars {
        return Err(AppError::BadRequest(format!(
            "{field_name} is too long. Keep it to {max_chars} characters or fewer."
        )));
    }

    Ok(())
}

fn validate_required_resource_categories(value: Option<&Value>) -> Result<(), AppError> {
    let items = require_nonempty_json_array(value, "required_resource_categories")?;

    if items.len() > MAX_REQUIRED_RESOURCE_CATEGORIES {
        return Err(AppError::BadRequest(format!(
            "required_resource_categories is capped at {MAX_REQUIRED_RESOURCE_CATEGORIES} items."
        )));
    }

    for (index, item) in items.iter().enumerate() {
        let category = item.as_str().ok_or_else(|| {
            AppError::BadRequest(format!(
                "required_resource_categories[{index}] must be a text category."
            ))
        })?;

        validate_resource_category(category, &format!("required_resource_categories[{index}]"))?;
    }

    Ok(())
}

fn validate_completion_criteria(value: Option<&Value>) -> Result<(), AppError> {
    let items = require_nonempty_json_array(value, "completion_criteria")?;

    if items.len() > MAX_COMPLETION_CRITERIA {
        return Err(AppError::BadRequest(format!(
            "completion_criteria is capped at {MAX_COMPLETION_CRITERIA} items."
        )));
    }

    for (index, item) in items.iter().enumerate() {
        let object = item.as_object().ok_or_else(|| {
            AppError::BadRequest(format!(
                "completion_criteria[{index}] must be a structured object."
            ))
        })?;

        let criterion_description = require_object_text(
            object,
            "completion_criteria",
            index,
            "criterion_description",
        )?;
        validate_required_text(
            criterion_description,
            &format!("completion_criteria[{index}].criterion_description"),
            MAX_COMPLETION_CRITERION_CHARS,
        )?;
        let status =
            require_object_text(object, "completion_criteria", index, "completion_status")?;
        if !is_valid_completion_status(status) {
            return Err(AppError::BadRequest(format!(
                "completion_criteria[{index}].completion_status must be one of: not_started, in_progress, completed, blocked."
            )));
        }
        validate_optional_object_text_max(
            object,
            "completion_criteria",
            index,
            "evidence_link",
            MAX_LINK_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "completion_criteria",
            index,
            "evidence_note",
            MAX_NOTE_CHARS,
        )?;
        validate_object_null_or_text_max(
            object,
            "completion_criteria",
            index,
            "updated_at",
            MAX_TIMESTAMP_CHARS,
        )?;
    }

    Ok(())
}

fn validate_execution_tracking_entries(value: Option<&Value>) -> Result<(), AppError> {
    let items = require_nonempty_json_array(value, "execution_tracking_entries")?;

    if items.len() > MAX_RESOURCE_REQUIREMENTS {
        return Err(AppError::BadRequest(format!(
            "execution_tracking_entries is capped at {MAX_RESOURCE_REQUIREMENTS} items."
        )));
    }

    for (index, item) in items.iter().enumerate() {
        let object = item.as_object().ok_or_else(|| {
            AppError::BadRequest(format!(
                "execution_tracking_entries[{index}] must be a structured object."
            ))
        })?;

        let category = require_object_text(
            object,
            "execution_tracking_entries",
            index,
            "resource_category",
        )?;
        validate_resource_category(
            category,
            &format!("execution_tracking_entries[{index}].resource_category"),
        )?;
        let target_needed =
            require_object_text(object, "execution_tracking_entries", index, "target_needed")?;
        validate_required_text(
            target_needed,
            &format!("execution_tracking_entries[{index}].target_needed"),
            MAX_RESOURCE_TARGET_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "target_amount",
            MAX_RESOURCE_AMOUNT_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "target_unit",
            MAX_RESOURCE_UNIT_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "current_acquired_amount",
            MAX_RESOURCE_AMOUNT_CHARS,
        )?;
        if let Some(status) = optional_object_text(
            object,
            "execution_tracking_entries",
            index,
            "resource_status",
        )? {
            if !is_valid_resource_status(status) {
                return Err(AppError::BadRequest(format!(
                    "execution_tracking_entries[{index}].resource_status must be one of: not_started, in_progress, secured, blocked."
                )));
            }
        }
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "external_coordination_link",
            MAX_LINK_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "status_proof_note",
            MAX_NOTE_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "resource_updated_at",
            MAX_TIMESTAMP_CHARS,
        )?;
    }

    Ok(())
}

fn require_nonempty_json_array<'a>(
    value: Option<&'a Value>,
    field_name: &str,
) -> Result<&'a Vec<Value>, AppError> {
    match value.and_then(Value::as_array) {
        Some(items) if !items.is_empty() => Ok(items),
        _ => Err(AppError::BadRequest(format!(
            "{field_name} must be a non-empty array."
        ))),
    }
}

fn require_object_text<'a>(
    object: &'a serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
) -> Result<&'a str, AppError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest(format!("{field_name}[{index}].{key} is required.")))
}

fn optional_object_text<'a>(
    object: &'a serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
) -> Result<Option<&'a str>, AppError> {
    match object.get(key) {
        Some(Value::String(value)) => Ok(Some(value.trim())),
        Some(Value::Null) | None => Ok(None),
        _ => Err(AppError::BadRequest(format!(
            "{field_name}[{index}].{key} must be a text field."
        ))),
    }
}

fn validate_optional_object_text_max(
    object: &serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    match object.get(key) {
        Some(Value::String(value)) => validate_text_max_chars(
            value.trim(),
            &format!("{field_name}[{index}].{key}"),
            max_chars,
        ),
        Some(Value::Null) | None => Ok(()),
        _ => Err(AppError::BadRequest(format!(
            "{field_name}[{index}].{key} must be a text field."
        ))),
    }
}

fn validate_object_null_or_text_max(
    object: &serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    match object.get(key) {
        Some(Value::Null) => Ok(()),
        Some(Value::String(value)) => validate_text_max_chars(
            value.trim(),
            &format!("{field_name}[{index}].{key}"),
            max_chars,
        ),
        _ => Err(AppError::BadRequest(format!(
            "{field_name}[{index}].{key} must be null or text."
        ))),
    }
}

fn validate_resource_category(value: &str, field_name: &str) -> Result<(), AppError> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(AppError::BadRequest(format!("{field_name} is required.")));
    }

    if is_supported_resource_category(&normalized) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "{field_name} must be one of: money, labor / manpower, skills / trades, materials, equipment, logistics / transport, organizational support, other."
        )))
    }
}

fn is_supported_resource_category(value: &str) -> bool {
    matches!(
        value,
        "money"
            | "labor"
            | "manpower"
            | "labor / manpower"
            | "skills"
            | "trades"
            | "skills / trades"
            | "materials"
            | "equipment"
            | "logistics"
            | "transport"
            | "logistics / transport"
            | "organizational support"
            | "other"
    )
}

fn is_valid_resource_status(value: &str) -> bool {
    matches!(value, "not_started" | "in_progress" | "secured" | "blocked")
}

fn is_valid_completion_status(value: &str) -> bool {
    matches!(
        value,
        "not_started" | "in_progress" | "completed" | "blocked"
    )
}

fn trimmed_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn required_moderation_note(value: Option<&String>) -> Result<String, AppError> {
    let note = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::BadRequest("A moderation note is required.".to_string()))?;

    validate_text_max_chars(&note, "Moderation note", MAX_NOTE_CHARS)?;

    Ok(note)
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read proposal data.".to_string())
}
