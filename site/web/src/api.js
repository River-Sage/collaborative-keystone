const defaultApiBase =
  typeof window === "undefined"
    ? "http://localhost:8080"
    : `${window.location.protocol}//${window.location.hostname}:8080`;

const API_BASE = (import.meta.env.VITE_API_BASE_URL || defaultApiBase).replace(
  /\/$/,
  ""
);

const CSRF_COOKIE_NAME = "ck_csrf";

async function apiFetch(path, options = {}) {
  const method = (options.method || "GET").toUpperCase();
  const csrfToken = needsCsrfHeader(method) ? readCookie(CSRF_COOKIE_NAME) : null;

  const response = await fetch(`${API_BASE}${path}`, {
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...(csrfToken ? { "X-CSRF-Token": csrfToken } : {}),
      ...(options.headers || {}),
    },
    ...options,
  });

  const contentType = response.headers.get("content-type") || "";
  const isJson = contentType.includes("application/json");
  const data = isJson ? await response.json() : await response.text();

  if (!response.ok) {
    const message =
      typeof data === "object" && data?.error
        ? data.error
        : typeof data === "string" && data.trim()
          ? data.trim()
        : `Request failed: ${response.status}`;

    const error = new Error(message);
    error.status = response.status;
    error.payload = data;
    throw error;
  }

  return data;
}

function needsCsrfHeader(method) {
  return !["GET", "HEAD", "OPTIONS"].includes(method);
}

function readCookie(name) {
  return document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${name}=`))
    ?.slice(name.length + 1) || "";
}

export const api = {
  publicUrl: (path) => `${API_BASE}${path}`,
  getSourceInfo: () => apiFetch("/source-info"),
  getBuildProvenance: () => apiFetch("/.well-known/keystone-build.json"),
  getLocaleRegistry: () => apiFetch("/.well-known/keystone-locales.json"),
  me: () => apiFetch("/auth/me"),
  login: (email, password) =>
    apiFetch("/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),
  register: (email, password, turnstileToken = "") =>
    apiFetch("/auth/register", {
      method: "POST",
      body: JSON.stringify({
        email,
        password,
        turnstile_token: turnstileToken || null,
      }),
    }),
  requestPasswordReset: (email, turnstileToken = "") =>
    apiFetch("/auth/password-reset/request", {
      method: "POST",
      body: JSON.stringify({
        email,
        turnstile_token: turnstileToken || null,
      }),
    }),
  confirmPasswordReset: (token, newPassword) =>
    apiFetch("/auth/password-reset/confirm", {
      method: "POST",
      body: JSON.stringify({ token, new_password: newPassword }),
    }),
  verifyEmail: (token) =>
    apiFetch("/auth/verify-email", {
      method: "POST",
      body: JSON.stringify({ token }),
    }),
  verifyEmailLink: (token) =>
    apiFetch("/auth/verify-email-link", {
      method: "POST",
      body: JSON.stringify({ token }),
    }),
  requestEmailVerificationToken: () =>
    apiFetch("/auth/email-verification-token", {
      method: "POST",
    }),
  logout: () =>
    apiFetch("/auth/logout", {
      method: "POST",
    }),
  getIssues: () => apiFetch("/proposals?board_code=issue"),
  getSolutions: () => apiFetch("/proposals?board_code=solution"),
  getArchive: () => apiFetch("/proposals?board_code=archive"),
  getProposal: (id) => apiFetch(`/proposals/${id}`),
  getProposalComments: (proposalId) => apiFetch(`/proposals/${proposalId}/comments`),
  createProposalComment: (proposalId, body) =>
    apiFetch(`/proposals/${proposalId}/comments`, {
      method: "POST",
      body: JSON.stringify({ body }),
    }),
  voteProposalComment: (proposalId, commentId, voteValue) =>
    apiFetch(`/proposals/${proposalId}/comments/${commentId}/vote`, {
      method: "POST",
      body: JSON.stringify({ vote_value: voteValue }),
    }),
  getExecutionRecords: () => apiFetch("/execution-records"),
  getExecutionRecord: (id) => apiFetch(`/execution-records/${id}`),
  updateExecutionRecord: (id, payload) =>
    apiFetch(`/execution-records/${id}`, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  getReviewPool: (boardCode, limit = 1) =>
    apiFetch(
      `/review-pool?board_code=${encodeURIComponent(boardCode)}&limit=${encodeURIComponent(limit)}`
    ),
  getReviewQueue: () => apiFetch("/review-queue"),
  getAntiAbuseQueue: () => apiFetch("/anti-abuse/review-queue"),
  resolveAntiAbuseFlag: (flagId, outcome, resolutionNote) =>
    apiFetch(`/anti-abuse/flags/${flagId}/resolve`, {
      method: "POST",
      body: JSON.stringify({
        outcome,
        resolution_note: resolutionNote,
      }),
    }),
  getCurrentCycleOutcomes: () => apiFetch("/cycle-outcomes/current"),
  resolveCurrentCycleOutcomes: () =>
    apiFetch("/cycle-outcomes/current", {
      method: "POST",
    }),
  getCycleResults: () => apiFetch("/cycle-results"),
  getAppealQueue: () => apiFetch("/appeals/review-queue"),
  getReconsiderationQueue: () => apiFetch("/reconsiderations/review-queue"),
  getUnlockStatus: (boardCode) =>
    apiFetch(`/me/unlock-status?board_code=${encodeURIComponent(boardCode)}`),
  getMyReviewQueues: () => apiFetch("/me/review-queues"),
  submitReviewAction: (proposalId, voteValue) =>
    apiFetch("/review-actions", {
      method: "POST",
      body: JSON.stringify({ proposal_id: proposalId, vote_value: voteValue }),
    }),
  castSentimentVote: (proposalId, voteValue) =>
    apiFetch(`/proposals/${proposalId}/votes/sentiment`, {
      method: "POST",
      body: JSON.stringify({ vote_value: voteValue }),
    }),
  castMergeVote: (proposalId, targetProposalId) =>
    apiFetch(`/proposals/${proposalId}/votes/merge`, {
      method: "POST",
      body: JSON.stringify({
        target_proposal_id: targetProposalId || null,
      }),
    }),
  upsertMergeDistinctionNote: (
    sourceProposalId,
    targetProposalId,
    differenceType,
    noteText
  ) =>
    apiFetch(`/proposals/${sourceProposalId}/merge-note`, {
      method: "POST",
      body: JSON.stringify({
        target_proposal_id: targetProposalId,
        difference_type: differenceType,
        note_text: noteText,
      }),
    }),
  createProposal: (payload) =>
    apiFetch("/proposals", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  submitAppeal: (proposalId, appealReason, clarificationNote) =>
    apiFetch(`/proposals/${proposalId}/appeal`, {
      method: "POST",
      body: JSON.stringify({
        appeal_reason: appealReason,
        clarification_note: clarificationNote,
      }),
    }),
  resolveAppeal: (appealId, outcome, moderatorNote) =>
    apiFetch(`/appeals/${appealId}/resolve`, {
      method: "POST",
      body: JSON.stringify({
        outcome,
        moderator_note: moderatorNote,
      }),
    }),
  startReconsideration: (proposalId, startReason, startNote) =>
    apiFetch(`/proposals/${proposalId}/reconsideration/start`, {
      method: "POST",
      body: JSON.stringify({
        start_reason: startReason,
        start_note: startNote,
      }),
    }),
  resolveReconsideration: (reconsiderationId, outcome, resolutionNote) =>
    apiFetch(`/reconsiderations/${reconsiderationId}/resolve`, {
      method: "POST",
      body: JSON.stringify({
        outcome,
        resolution_note: resolutionNote,
      }),
    }),
  createExecutionRecord: (solutionProposalId) =>
    apiFetch(`/proposals/${solutionProposalId}/execution-record`, {
      method: "POST",
    }),
  moderateArchive: (proposalId, archivedReason, moderationNote) =>
    apiFetch("/proposals/moderate-archive", {
      method: "POST",
      body: JSON.stringify({
        proposal_id: proposalId,
        archived_reason: archivedReason,
        moderation_note: moderationNote,
      }),
    }),
  moderateFreeze: (proposalId, moderationNote) =>
    apiFetch("/proposals/moderate-freeze", {
      method: "POST",
      body: JSON.stringify({
        proposal_id: proposalId,
        moderation_note: moderationNote,
      }),
    }),
  moderateUnfreeze: (proposalId, moderationNote) =>
    apiFetch("/proposals/moderate-unfreeze", {
      method: "POST",
      body: JSON.stringify({
        proposal_id: proposalId,
        moderation_note: moderationNote,
      }),
    }),
  moderateReviewedActive: (proposalId, moderationNote) =>
    apiFetch("/proposals/moderate-reviewed-active", {
      method: "POST",
      body: JSON.stringify({
        proposal_id: proposalId,
        moderation_note: moderationNote,
      }),
    }),
  executeMerge: (sourceProposalId, targetProposalId) =>
    apiFetch("/proposals/merge-execute", {
      method: "POST",
      body: JSON.stringify({
        source_proposal_id: sourceProposalId,
        target_proposal_id: targetProposalId,
      }),
    }),
};
