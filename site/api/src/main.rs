use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, header},
    middleware,
    routing::{get, post},
};
use dotenvy::dotenv;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use anti_abuse::{anti_abuse_review_queue_handler, resolve_anti_abuse_flag_handler};
use appeals::{appeal_review_queue_handler, resolve_appeal_handler, submit_appeal_handler};
use executions::{
    create_execution_record_handler, get_execution_record_handler, list_execution_records_handler,
    update_execution_record_handler,
};
use merge_notes::{get_merge_relationship_handler, upsert_merge_distinction_note_handler};
use reconsiderations::{
    reconsideration_review_queue_handler, resolve_reconsideration_handler,
    start_reconsideration_handler,
};

mod anti_abuse;
mod appeals;
mod auth;
mod csrf;
mod cycles;
mod error;
mod executions;
mod mail;
mod merge_notes;
mod my_queue;
mod notifications;
mod proposals;
mod rate_limit;
mod reconsiderations;
mod review_actions;
mod votes;

use auth::{
    email_verification_token_handler, login_handler, logout_handler, me_handler,
    password_reset_confirm_handler, password_reset_request_handler, register_handler,
    verify_email_handler,
};
use my_queue::my_review_queues_handler;
use proposals::{
    create_proposal_handler, current_cycle_outcome_handler, execute_merge_handler,
    get_proposal_handler, list_proposals_handler, moderate_archive_handler,
    moderate_freeze_handler, moderate_reviewed_active_handler, moderate_unfreeze_handler,
    published_cycle_results_handler, resolve_current_cycle_outcomes_handler, review_pool_handler,
    review_queue_handler,
};
use review_actions::{submit_review_action_handler, unlock_status_handler};
use votes::{cast_merge_vote_handler, cast_sentiment_vote_handler};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub mailer: mail::Mailer,
    pub rate_limiter: rate_limit::RateLimiter,
}

async fn health() -> &'static str {
    "ok"
}

const DEFAULT_CORS_ORIGINS: [&str; 4] = [
    "http://localhost:5173",
    "http://127.0.0.1:5173",
    "http://localhost:5174",
    "http://127.0.0.1:5174",
];
const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "api=debug,tower_http=debug".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set in api/.env or the shell");
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a valid integer");
    let mailer = mail::Mailer::from_env(is_production_environment())
        .map_err(|err| format!("mail configuration error: {err}"))?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("../db/migrations").run(&db).await?;
    sqlx::query("SELECT 1").execute(&db).await?;
    cycles::ensure_active_world_cycle(&db).await?;
    auth::seed_development_accounts(&db).await;

    let state = Arc::new(AppState {
        db,
        mailer,
        rate_limiter: rate_limit::RateLimiter::new(),
    });

    let app = Router::new()
        .route("/me/review-queues", get(my_review_queues_handler))
        .route(
            "/proposals/moderate-archive",
            post(moderate_archive_handler),
        )
        .route("/proposals/moderate-freeze", post(moderate_freeze_handler))
        .route(
            "/proposals/moderate-unfreeze",
            post(moderate_unfreeze_handler),
        )
        .route(
            "/proposals/moderate-reviewed-active",
            post(moderate_reviewed_active_handler),
        )
        .route("/appeals/review-queue", get(appeal_review_queue_handler))
        .route("/appeals/{id}/resolve", post(resolve_appeal_handler))
        .route(
            "/reconsiderations/review-queue",
            get(reconsideration_review_queue_handler),
        )
        .route(
            "/reconsiderations/{id}/resolve",
            post(resolve_reconsideration_handler),
        )
        .route("/review-queue", get(review_queue_handler))
        .route(
            "/anti-abuse/review-queue",
            get(anti_abuse_review_queue_handler),
        )
        .route(
            "/anti-abuse/flags/{id}/resolve",
            post(resolve_anti_abuse_flag_handler),
        )
        .route(
            "/cycle-outcomes/current",
            get(current_cycle_outcome_handler).post(resolve_current_cycle_outcomes_handler),
        )
        .route("/cycle-results", get(published_cycle_results_handler))
        .route("/health", get(health))
        .route("/execution-records", get(list_execution_records_handler))
        .route(
            "/execution-records/{id}",
            get(get_execution_record_handler).post(update_execution_record_handler),
        )
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        .route(
            "/auth/password-reset/request",
            post(password_reset_request_handler),
        )
        .route(
            "/auth/password-reset/confirm",
            post(password_reset_confirm_handler),
        )
        .route("/auth/logout", post(logout_handler))
        .route("/auth/verify-email", post(verify_email_handler))
        .route(
            "/auth/email-verification-token",
            post(email_verification_token_handler),
        )
        .route("/auth/me", get(me_handler))
        .route(
            "/proposals",
            post(create_proposal_handler).get(list_proposals_handler),
        )
        .route("/proposals/{id}", get(get_proposal_handler))
        .route("/proposals/{id}/appeal", post(submit_appeal_handler))
        .route(
            "/proposals/{id}/reconsideration/start",
            post(start_reconsideration_handler),
        )
        .route(
            "/proposals/{id}/execution-record",
            post(create_execution_record_handler),
        )
        .route(
            "/proposals/{id}/votes/sentiment",
            post(cast_sentiment_vote_handler),
        )
        .route("/proposals/{id}/votes/merge", post(cast_merge_vote_handler))
        .route(
            "/proposals/{id}/merge-note",
            post(upsert_merge_distinction_note_handler),
        )
        .route("/proposals/merge-execute", post(execute_merge_handler))
        .route(
            "/merge-relationships/{source_id}/{target_id}",
            get(get_merge_relationship_handler),
        )
        .route("/review-pool", get(review_pool_handler))
        .route("/review-actions", post(submit_review_action_handler))
        .route("/me/unlock-status", get(unlock_status_handler))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(configured_cors_origins()))
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([
                    header::CONTENT_TYPE,
                    HeaderName::from_static(csrf::CSRF_HEADER_NAME),
                ])
                .allow_credentials(true),
        )
        .layer(middleware::from_fn(csrf::validate_csrf))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("API listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

fn configured_cors_origins() -> Vec<HeaderValue> {
    let configured = env::var("CORS_ALLOWED_ORIGINS")
        .ok()
        .or_else(|| env::var("WEB_ORIGIN").ok());

    if is_production_environment()
        && configured
            .as_deref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    {
        panic!("CORS_ALLOWED_ORIGINS or WEB_ORIGIN must be set in production");
    }

    let raw_origins = configured.unwrap_or_else(|| DEFAULT_CORS_ORIGINS.join(","));
    let origins: Vec<HeaderValue> = raw_origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            validate_cors_origin(origin);
            origin.parse::<HeaderValue>().unwrap_or_else(|_| {
                panic!("CORS origin '{origin}' is not a valid HTTP header value")
            })
        })
        .collect();

    if origins.is_empty() {
        panic!("At least one CORS origin must be configured");
    }

    origins
}

fn validate_cors_origin(origin: &str) {
    if is_production_environment() && !origin.starts_with("https://") {
        panic!("Production CORS origin '{origin}' must use https://");
    }
}

fn is_production_environment() -> bool {
    for key in ["APP_ENV", "RUST_ENV"] {
        if let Ok(value) = env::var(key) {
            if value.trim().eq_ignore_ascii_case("production") {
                return true;
            }
        }
    }

    false
}
