use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState, anti_abuse, auth::AuthUser, error::AppError,
    reconsiderations::resolve_cleared_reconsiderations, review_actions::ensure_review_unlocked,
};

const MAX_COMMENT_CHARS: usize = 1000;

#[derive(Debug, Serialize)]
pub struct CommentListResponse {
    pub ok: bool,
    pub comments: Vec<CommentSummary>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct CreateCommentResponse {
    pub ok: bool,
    pub comment: CommentSummary,
}

#[derive(Debug, Deserialize)]
pub struct VoteCommentRequest {
    pub vote_value: String,
}

#[derive(Debug, Serialize)]
pub struct VoteCommentResponse {
    pub ok: bool,
    pub comment_id: Uuid,
    pub current_user_vote: String,
}

#[derive(Debug, Serialize)]
pub struct CommentSummary {
    pub id: Uuid,
    pub body: String,
    pub author_label: Option<String>,
    pub current_user_comment: bool,
    pub current_user_vote: Option<String>,
}

struct DiscussionProposalContext {
    board_code: String,
    primary_state: String,
}

pub async fn list_comments_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(proposal_id): Path<Uuid>,
) -> Result<Json<CommentListResponse>, AppError> {
    resolve_cleared_reconsiderations(&state.db).await?;
    load_discussion_proposal_context(&state.db, &state.locale.slug, proposal_id).await?;

    let comments = load_comments(&state.db, proposal_id, auth_user.user_id).await?;

    Ok(Json(CommentListResponse { ok: true, comments }))
}

pub async fn create_comment_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
    Path(proposal_id): Path<Uuid>,
    Json(payload): Json<CreateCommentRequest>,
) -> Result<(StatusCode, Json<CreateCommentResponse>), AppError> {
    auth_user.require_verified()?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let proposal =
        load_discussion_proposal_context(&state.db, &state.locale.slug, proposal_id).await?;
    ensure_can_discuss(&state.db, auth_user.user_id, &proposal).await?;

    let body = validate_comment_body(&payload.body)?;
    let mut tx = state.db.begin().await.map_err(|err| {
        error!("database error starting comment transaction: {}", err);
        AppError::Internal("Failed to save comment.".to_string())
    })?;

    let inserted = sqlx::query(
        r#"
        INSERT INTO proposal_comments (proposal_id, author_user_id, body)
        VALUES ($1, $2, $3)
        ON CONFLICT (proposal_id, author_user_id)
        DO NOTHING
        RETURNING id
        "#,
    )
    .bind(proposal_id)
    .bind(auth_user.user_id)
    .bind(&body)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error inserting proposal comment: {}", err);
        AppError::Internal("Failed to save comment.".to_string())
    })?;

    let Some(inserted) = inserted else {
        return Err(AppError::BadRequest(
            "You already commented on this submission.".to_string(),
        ));
    };

    let comment_id: Uuid = inserted.try_get("id").map_err(internal_db_err)?;

    sqlx::query(
        r#"
        INSERT INTO proposal_comment_votes (comment_id, user_id, vote_value)
        VALUES ($1, $2, 'like')
        "#,
    )
    .bind(comment_id)
    .bind(auth_user.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error inserting initial comment vote: {}", err);
        AppError::Internal("Failed to save comment.".to_string())
    })?;

    tx.commit().await.map_err(|err| {
        error!("database error committing comment transaction: {}", err);
        AppError::Internal("Failed to save comment.".to_string())
    })?;

    anti_abuse::record_user_activity(
        &state.db,
        auth_user.user_id,
        "proposal_comment_created",
        Some(proposal_id),
        None,
        &headers,
        json!({
            "comment_id": comment_id,
            "board_code": proposal.board_code
        }),
    )
    .await?;

    let comment = load_comment(&state.db, proposal_id, comment_id, auth_user.user_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateCommentResponse { ok: true, comment }),
    ))
}

pub async fn vote_comment_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
    Path((proposal_id, comment_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<VoteCommentRequest>,
) -> Result<Json<VoteCommentResponse>, AppError> {
    auth_user.require_verified()?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let vote_value = payload.vote_value.trim().to_lowercase();
    if vote_value != "like" && vote_value != "dislike" {
        return Err(AppError::BadRequest(
            "vote_value must be either like or dislike.".to_string(),
        ));
    }

    let proposal =
        load_discussion_proposal_context(&state.db, &state.locale.slug, proposal_id).await?;
    ensure_can_discuss(&state.db, auth_user.user_id, &proposal).await?;
    ensure_active_comment_belongs_to_proposal(&state.db, proposal_id, comment_id).await?;

    sqlx::query(
        r#"
        INSERT INTO proposal_comment_votes (comment_id, user_id, vote_value)
        VALUES ($1, $2, $3)
        ON CONFLICT (comment_id, user_id)
        DO UPDATE SET
            vote_value = EXCLUDED.vote_value,
            updated_at = NOW()
        "#,
    )
    .bind(comment_id)
    .bind(auth_user.user_id)
    .bind(&vote_value)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error upserting comment vote: {}", err);
        AppError::Internal("Failed to save comment vote.".to_string())
    })?;

    anti_abuse::record_user_activity(
        &state.db,
        auth_user.user_id,
        "proposal_comment_vote",
        Some(proposal_id),
        None,
        &headers,
        json!({
            "comment_id": comment_id,
            "vote_value": vote_value,
            "board_code": proposal.board_code
        }),
    )
    .await?;

    Ok(Json(VoteCommentResponse {
        ok: true,
        comment_id,
        current_user_vote: vote_value,
    }))
}

async fn load_discussion_proposal_context(
    db: &sqlx::PgPool,
    locale_slug: &str,
    proposal_id: Uuid,
) -> Result<DiscussionProposalContext, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            b.code AS board_code,
            p.primary_state
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = $2
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .bind(locale_slug)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading discussion proposal context: {}",
            err
        );
        AppError::Internal("Failed to load discussion.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    Ok(DiscussionProposalContext {
        board_code: row.try_get("board_code").map_err(internal_db_err)?,
        primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
    })
}

async fn ensure_can_discuss(
    db: &sqlx::PgPool,
    user_id: Uuid,
    proposal: &DiscussionProposalContext,
) -> Result<(), AppError> {
    if proposal.primary_state != "active" {
        return Err(AppError::BadRequest(
            "Discussion is only open on active submissions.".to_string(),
        ));
    }

    if proposal.board_code != "issue" && proposal.board_code != "solution" {
        return Err(AppError::BadRequest(
            "Discussion is only available on issue and solution submissions.".to_string(),
        ));
    }

    ensure_review_unlocked(db, user_id, Some(&proposal.board_code)).await
}

async fn ensure_active_comment_belongs_to_proposal(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    comment_id: Uuid,
) -> Result<(), AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM proposal_comments
            WHERE id = $1
              AND proposal_id = $2
              AND state = 'active'
        ) AS exists_flag
        "#,
    )
    .bind(comment_id)
    .bind(proposal_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking proposal comment: {}", err);
        AppError::Internal("Failed to save comment vote.".to_string())
    })?;

    let exists_flag: bool = row.try_get("exists_flag").map_err(internal_db_err)?;
    if exists_flag {
        Ok(())
    } else {
        Err(AppError::BadRequest("Comment not found.".to_string()))
    }
}

async fn load_comments(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<CommentSummary>, AppError> {
    let rows = sqlx::query(comment_list_sql())
        .bind(proposal_id)
        .bind(user_id)
        .fetch_all(db)
        .await
        .map_err(|err| {
            error!("database error loading proposal comments: {}", err);
            AppError::Internal("Failed to load discussion.".to_string())
        })?;

    rows.into_iter().map(map_comment_row).collect()
}

async fn load_comment(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    comment_id: Uuid,
    user_id: Uuid,
) -> Result<CommentSummary, AppError> {
    let row = sqlx::query(
        r#"
        WITH vote_stats AS (
            SELECT
                cv.comment_id,
                COUNT(*) FILTER (WHERE cv.vote_value = 'like')::int AS like_count,
                COUNT(*) FILTER (WHERE cv.vote_value = 'dislike')::int AS dislike_count,
                COUNT(*)::int AS total_count
            FROM proposal_comment_votes cv
            GROUP BY cv.comment_id
        )
        SELECT
            c.id,
            c.body,
            CASE WHEN c.author_user_id = p.author_user_id THEN 'Author' ELSE NULL END AS author_label,
            c.author_user_id = $2 AS current_user_comment,
            user_vote.vote_value AS current_user_vote
        FROM proposal_comments c
        JOIN proposals p ON p.id = c.proposal_id
        LEFT JOIN proposal_comment_votes user_vote
          ON user_vote.comment_id = c.id
         AND user_vote.user_id = $2
        LEFT JOIN vote_stats stats ON stats.comment_id = c.id
        WHERE c.proposal_id = $1
          AND c.id = $3
          AND c.state = 'active'
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .bind(user_id)
    .bind(comment_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error loading proposal comment: {}", err);
        AppError::Internal("Failed to load discussion.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest("Comment not found.".to_string()));
    };

    map_comment_row(row)
}

fn comment_list_sql() -> &'static str {
    r#"
    WITH vote_stats AS (
        SELECT
            cv.comment_id,
            COUNT(*) FILTER (WHERE cv.vote_value = 'like')::int AS like_count,
            COUNT(*) FILTER (WHERE cv.vote_value = 'dislike')::int AS dislike_count,
            COUNT(*)::int AS total_count
        FROM proposal_comment_votes cv
        GROUP BY cv.comment_id
    )
    SELECT
        c.id,
        c.body,
        CASE WHEN c.author_user_id = p.author_user_id THEN 'Author' ELSE NULL END AS author_label,
        c.author_user_id = $2 AS current_user_comment,
        user_vote.vote_value AS current_user_vote
    FROM proposal_comments c
    JOIN proposals p ON p.id = c.proposal_id
    LEFT JOIN proposal_comment_votes user_vote
      ON user_vote.comment_id = c.id
     AND user_vote.user_id = $2
    LEFT JOIN vote_stats stats ON stats.comment_id = c.id
    WHERE c.proposal_id = $1
      AND c.state = 'active'
    ORDER BY
      CASE
        WHEN COALESCE(stats.total_count, 0) > 0
        THEN COALESCE(stats.like_count, 0)::numeric / stats.total_count::numeric
        ELSE 0
      END DESC,
      (COALESCE(stats.like_count, 0) - COALESCE(stats.dislike_count, 0)) DESC,
      COALESCE(stats.total_count, 0) DESC,
      c.created_at ASC
    "#
}

fn map_comment_row(row: sqlx::postgres::PgRow) -> Result<CommentSummary, AppError> {
    Ok(CommentSummary {
        id: row.try_get("id").map_err(internal_db_err)?,
        body: row.try_get("body").map_err(internal_db_err)?,
        author_label: row.try_get("author_label").map_err(internal_db_err)?,
        current_user_comment: row
            .try_get("current_user_comment")
            .map_err(internal_db_err)?,
        current_user_vote: row.try_get("current_user_vote").map_err(internal_db_err)?,
    })
}

fn validate_comment_body(value: &str) -> Result<String, AppError> {
    let body = value.trim().to_string();
    if body.is_empty() {
        return Err(AppError::BadRequest("Comment is required.".to_string()));
    }

    if body.chars().count() > MAX_COMMENT_CHARS {
        return Err(AppError::BadRequest(format!(
            "Comment is too long. Keep it to {MAX_COMMENT_CHARS} characters or fewer."
        )));
    }

    Ok(body)
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("database row mapping error: {}", err);
    AppError::Internal("Unexpected database response.".to_string())
}
