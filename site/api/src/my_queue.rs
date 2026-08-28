use std::sync::Arc;

use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;
use serde::Serialize;
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState, auth::AuthUser, error::AppError, reconsiderations::resolve_cleared_reconsiderations,
};

#[derive(Debug, Serialize, Clone)]
pub struct MyQueueProposal {
    pub id: Uuid,
    pub board_code: String,
    pub title: String,
    #[serde(skip_serializing)]
    pub eligible_for_review_unlock: bool,
    pub is_archived: bool,

    pub problem_description: Option<String>,
    pub affected_scope: Option<String>,
    pub why_it_matters: Option<String>,
    pub action_description: Option<String>,

    pub current_user_sentiment_vote: Option<String>,
    pub current_user_merge_vote_present: bool,
    pub current_user_merge_target_proposal_id: Option<Uuid>,
    pub current_user_reviewed: bool,

    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MyReviewQueuesResponse {
    pub ok: bool,
    pub issues_to_review: Vec<MyQueueProposal>,
    pub solutions_to_review: Vec<MyQueueProposal>,
    pub issues_reviewed: Vec<MyQueueProposal>,
    pub solutions_reviewed: Vec<MyQueueProposal>,
}

pub async fn my_review_queues_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<MyReviewQueuesResponse>, AppError> {
    resolve_cleared_reconsiderations(&state.db).await?;

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
            p.problem_description,
            p.affected_scope,
            p.why_it_matters,
            p.action_description,
            sv.vote_value AS current_user_sentiment_vote,
            CASE WHEN mv.id IS NULL THEN FALSE ELSE TRUE END AS current_user_merge_vote_present,
            mv.target_proposal_id AS current_user_merge_target_proposal_id,
            CASE
                WHEN sv.id IS NOT NULL THEN TRUE
                WHEN mv.id IS NOT NULL THEN TRUE
                WHEN ra.id IS NOT NULL THEN TRUE
                ELSE FALSE
            END AS current_user_reviewed,
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
                  AND rw.ends_at <= NOW()
                  AND (
                    p.unsafe_count >= 8
                    OR (
                        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                        AND p.unsafe_count::numeric
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
            ) AS reconsideration_moderation_due,
            p.created_at
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        LEFT JOIN proposal_sentiment_votes sv
            ON sv.proposal_id = p.id
           AND sv.user_id = $1
        LEFT JOIN proposal_merge_votes mv
            ON mv.proposal_id = p.id
           AND mv.user_id = $1
        LEFT JOIN review_actions ra
            ON ra.proposal_id = p.id
           AND ra.user_id = $1
        WHERE l.slug = $2
          AND c.is_active = TRUE
          AND p.author_user_id <> $1
          AND b.code IN ('issue', 'solution')
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(auth_user.user_id)
    .bind(&state.locale.slug)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading my review queues: {}", err);
        AppError::Internal("Failed to load personal review queues.".to_string())
    })?;

    let proposals = rows
        .into_iter()
        .map(map_my_queue_row)
        .collect::<Result<Vec<_>, AppError>>()?;

    let mut issues_to_review = Vec::new();
    let mut solutions_to_review = Vec::new();
    let mut issues_reviewed = Vec::new();
    let mut solutions_reviewed = Vec::new();

    for proposal in proposals {
        let is_issue = proposal.board_code == "issue";
        let is_solution = proposal.board_code == "solution";
        let eligible_for_review_unlock = proposal.eligible_for_review_unlock;

        if proposal.current_user_reviewed {
            if is_issue {
                issues_reviewed.push(proposal);
            } else if is_solution {
                solutions_reviewed.push(proposal);
            }
        } else if eligible_for_review_unlock {
            if is_issue {
                issues_to_review.push(proposal);
            } else if is_solution {
                solutions_to_review.push(proposal);
            }
        }
    }

    let mut rng = rand::thread_rng();
    issues_to_review.shuffle(&mut rng);
    solutions_to_review.shuffle(&mut rng);

    Ok(Json(MyReviewQueuesResponse {
        ok: true,
        issues_to_review,
        solutions_to_review,
        issues_reviewed,
        solutions_reviewed,
    }))
}

fn map_my_queue_row(row: sqlx::postgres::PgRow) -> Result<MyQueueProposal, AppError> {
    let primary_state: String = row.try_get("primary_state").map_err(internal_db_err)?;
    let frozen_for_review: bool = row.try_get("frozen_for_review").map_err(internal_db_err)?;
    let reconsideration_moderation_due: bool = row
        .try_get("reconsideration_moderation_due")
        .map_err(internal_db_err)?;
    let support_count: i32 = row.try_get("support_count").map_err(internal_db_err)?;
    let not_a_fit_count: i32 = row.try_get("not_a_fit_count").map_err(internal_db_err)?;
    let unclear_count: i32 = row.try_get("unclear_count").map_err(internal_db_err)?;
    let unsafe_count: i32 = row.try_get("unsafe_count").map_err(internal_db_err)?;
    let merge_count: i32 = row.try_get("merge_count").map_err(internal_db_err)?;
    let is_active =
        primary_state == "active" && !frozen_for_review && !reconsideration_moderation_due;
    let eligible_for_review_unlock = is_active
        && !excluded_from_review_credit(
            support_count,
            not_a_fit_count,
            unclear_count,
            unsafe_count,
            merge_count,
        );

    Ok(MyQueueProposal {
        id: row.try_get("id").map_err(internal_db_err)?,
        board_code: row.try_get("board_code").map_err(internal_db_err)?,
        title: row.try_get("title").map_err(internal_db_err)?,
        eligible_for_review_unlock,
        is_archived: primary_state == "archived",

        problem_description: row
            .try_get("problem_description")
            .map_err(internal_db_err)?,
        affected_scope: row.try_get("affected_scope").map_err(internal_db_err)?,
        why_it_matters: row.try_get("why_it_matters").map_err(internal_db_err)?,
        action_description: row.try_get("action_description").map_err(internal_db_err)?,

        current_user_sentiment_vote: row
            .try_get("current_user_sentiment_vote")
            .map_err(internal_db_err)?,
        current_user_merge_vote_present: row
            .try_get("current_user_merge_vote_present")
            .map_err(internal_db_err)?,
        current_user_merge_target_proposal_id: row
            .try_get("current_user_merge_target_proposal_id")
            .map_err(internal_db_err)?,
        current_user_reviewed: row
            .try_get("current_user_reviewed")
            .map_err(internal_db_err)?,

        created_at: row.try_get("created_at").map_err(internal_db_err)?,
    })
}

fn excluded_from_review_credit(
    support_count: i32,
    not_a_fit_count: i32,
    unclear_count: i32,
    unsafe_count: i32,
    merge_count: i32,
) -> bool {
    let negative_count = not_a_fit_count + unclear_count + unsafe_count;
    let total_count = support_count + negative_count + merge_count;

    negative_count > 8 * support_count.max(1)
        || unsafe_count >= 8
        || fraction_at_least(unsafe_count, total_count, 0.50)
}

fn fraction_at_least(part: i32, total: i32, threshold: f64) -> bool {
    total > 0 && (part as f64 / total as f64) >= threshold
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read personal queue data.".to_string())
}
