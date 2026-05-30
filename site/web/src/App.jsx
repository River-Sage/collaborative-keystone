import { useEffect, useMemo, useState } from "react";
import { api } from "./api";
import "./App.css";

const TABS = [
  { key: "issues", label: "Issues" },
  { key: "solutions", label: "Solutions" },
  { key: "implementations", label: "Implementations" },
  { key: "outcomes", label: "Outcomes", moderatorOnly: true },
  { key: "reviewQueue", label: "Review Queue", moderatorOnly: true },
  { key: "trustReview", label: "Trust Review", moderatorOnly: true },
  { key: "appeals", label: "Appeals", moderatorOnly: true },
  { key: "reconsiderations", label: "Reconsideration", moderatorOnly: true },
  { key: "archive", label: "Archive" },
  { key: "account", label: "Account" },
];

const PRIMARY_NAV_TAB_KEYS = new Set(["issues", "solutions"]);
const MODERATOR_ROLES = new Set(["moderator"]);
const STANDARD_REQUIRED_REVIEW_COUNT = 4;
const REQUIRED_REVIEW_DISPLAY_LIMIT = 1;
const MAX_COMPLETION_CRITERIA = 8;
const MAX_RESOURCE_REQUIREMENTS = 64;
const MAX_TITLE_CHARS = 120;
const MAX_SCOPE_CHARS = 500;
const MAX_LONG_TEXT_CHARS = 2000;
const MAX_SOLUTION_FIT_CHARS = 1000;
const MAX_COMPLETION_CRITERION_CHARS = 240;
const MAX_RESOURCE_AMOUNT_CHARS = 64;
const MAX_RESOURCE_UNIT_CHARS = 64;
const MAX_NOTE_CHARS = 2000;
const MAX_LINK_CHARS = 2048;
const MAX_EMAIL_CHARS = 254;
const MAX_PASSWORD_CHARS = 128;
const MAX_TOKEN_CHARS = 128;
const MAX_SEARCH_CHARS = 200;
const SUBMISSION_ID_CHARS = 36;
const SHOW_PROTOTYPE_ACCOUNTS = import.meta.env.DEV;
const PROTOTYPE_PASSWORD = SHOW_PROTOTYPE_ACCOUNTS
  ? "SuperSecurePass123"
  : "";
const PROTOTYPE_ACCOUNTS = SHOW_PROTOTYPE_ACCOUNTS ? [
  { email: "user@example.com", roleLabel: "User", password: PROTOTYPE_PASSWORD },
  {
    email: "moderator@example.com",
    roleLabel: "Moderator",
    password: PROTOTYPE_PASSWORD,
  },
] : [];
const DEFAULT_AUTH_EMAIL = SHOW_PROTOTYPE_ACCOUNTS ? "user@example.com" : "";

const VOTE_OPTIONS = [
  { value: "support", label: "Support" },
  { value: "not_a_fit", label: "Not a Fit" },
  { value: "unclear", label: "Unclear" },
  { value: "unsafe", label: "Unsafe / Illegal / Deceptive" },
];

const DIFFERENCE_TYPE_OPTIONS = [
  { value: "different_scope", label: "Different Scope" },
  { value: "different_cause", label: "Different Cause" },
  { value: "different_affected_group", label: "Different Affected Group" },
  { value: "different_implementation", label: "Different Implementation" },
  { value: "different_completion_criteria", label: "Different Criteria" },
  { value: "other", label: "Other" },
];

const RESOURCE_CATEGORY_OPTIONS = [
  { value: "money", label: "Money" },
  { value: "labor / manpower", label: "People / Labor" },
  { value: "skills / trades", label: "Specialized Skills" },
  { value: "materials", label: "Materials" },
  { value: "equipment", label: "Equipment" },
  { value: "logistics / transport", label: "Logistics / Transport" },
  { value: "organizational support", label: "Coordination / Permissions" },
  { value: "other", label: "Other" },
];

const RESOURCE_CATEGORY_LABELS = {
  ...Object.fromEntries(
    RESOURCE_CATEGORY_OPTIONS.map((option) => [option.value, option.label])
  ),
  labor: "People / Labor",
  manpower: "People / Labor",
  skills: "Specialized Skills",
  trades: "Specialized Skills",
  logistics: "Logistics / Transport",
  transport: "Logistics / Transport",
};

function createEmptyCompletionCriterion(description = "") {
  return {
    criterion_description: description,
    completion_status: "not_started",
    evidence_note: "",
    updated_at: null,
  };
}

function createEmptyExecutionEntry(overrides = {}) {
  return {
    resource_category: "organizational support",
    target_needed: "",
    target_amount: "",
    target_unit: "",
    current_acquired_amount: "",
    external_coordination_link: "",
    status_proof_note: "",
    ...overrides,
  };
}

const emptyIssueForm = {
  title: "",
  problemDescription: "",
  affectedScope: "",
  whyItMatters: "",
};

const emptySolutionForm = {
  title: "",
  parentIssueProposalId: "",
  actionDescription: "",
  whyThisSolvesIt: "",
  completionCriteria: [createEmptyCompletionCriterion()],
  executionTrackingEntries: [createEmptyExecutionEntry()],
};

const emptyReviewQueues = {
  issues_to_review: [],
  solutions_to_review: [],
  issues_reviewed: [],
  solutions_reviewed: [],
};

function formatRoleLabel(roleCode) {
  if (roleCode === "moderator") return "Moderator";
  if (roleCode === "registered_user") return "User";
  return roleCode || "User";
}

function getProposalTotalCount(proposal) {
  if (!proposal) return 0;
  return (
    (proposal.support_count || 0) +
    (proposal.not_a_fit_count || 0) +
    (proposal.unclear_count || 0) +
    (proposal.unsafe_count || 0) +
    (proposal.merge_count || 0)
  );
}

function isHighModerationWatch(proposal) {
  if (!proposal) return false;
  const totalCount = getProposalTotalCount(proposal);
  const unsafeCount = proposal.unsafe_count || 0;
  return unsafeCount >= 8 || (totalCount > 0 && unsafeCount / totalCount >= 0.35);
}

function isMergeWatch(proposal) {
  if (!proposal) return false;
  const totalCount = getProposalTotalCount(proposal);
  const mergeCount = proposal.merge_count || 0;
  return totalCount >= 10 && mergeCount / totalCount >= 0.2;
}

function getVoteOptionLabel(option, boardCode) {
  if (boardCode === "issue" && option.value === "not_a_fit") {
    return "Pass";
  }

  return option.label;
}

function isUuid(value) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
    value.trim()
  );
}

function App() {
  const [me, setMe] = useState(null);
  const [sessionChecked, setSessionChecked] = useState(false);

  const [authMode, setAuthMode] = useState("login");
  const [email, setEmail] = useState(DEFAULT_AUTH_EMAIL);
  const [password, setPassword] = useState(PROTOTYPE_PASSWORD);
  const [authLoading, setAuthLoading] = useState(false);
  const [authError, setAuthError] = useState("");
  const [authSuccess, setAuthSuccess] = useState("");
  const [passwordResetToken, setPasswordResetToken] = useState("");
  const [passwordResetNewPassword, setPasswordResetNewPassword] = useState("");
  const [verificationToken, setVerificationToken] = useState("");
  const [verificationLoading, setVerificationLoading] = useState(false);
  const [verificationError, setVerificationError] = useState("");
  const [verificationSuccess, setVerificationSuccess] = useState("");

  const [activeTab, setActiveTab] = useState("issues");
  const [requiredReviewBoard, setRequiredReviewBoard] = useState("issue");

  const [items, setItems] = useState([]);
  const [itemsLoading, setItemsLoading] = useState(true);
  const [itemsError, setItemsError] = useState("");

  const [selectedProposal, setSelectedProposal] = useState(null);
  const [selectedProposalLoading, setSelectedProposalLoading] = useState(false);
  const [selectedProposalError, setSelectedProposalError] = useState("");
  const [selectedExecution, setSelectedExecution] = useState(null);
  const [selectedExecutionLoading, setSelectedExecutionLoading] = useState(false);
  const [selectedExecutionError, setSelectedExecutionError] = useState("");

  const [navDrawerOpen, setNavDrawerOpen] = useState(false);
  const [detailDrawerOpen, setDetailDrawerOpen] = useState(false);
  const [frontDrawer, setFrontDrawer] = useState("detail");

  const [moderationReason, setModerationReason] = useState("irrelevant");
  const [moderationNote, setModerationNote] = useState("");
  const [moderationLoading, setModerationLoading] = useState(false);
  const [moderationError, setModerationError] = useState("");
  const [moderationSuccess, setModerationSuccess] = useState("");

  const [mergeTargetId, setMergeTargetId] = useState("");
  const [mergeLoading, setMergeLoading] = useState(false);
  const [mergeError, setMergeError] = useState("");
  const [mergeSuccess, setMergeSuccess] = useState("");
  const [mergeVoteTargetId, setMergeVoteTargetId] = useState("");
  const [executionLoading, setExecutionLoading] = useState(false);
  const [executionError, setExecutionError] = useState("");
  const [executionSuccess, setExecutionSuccess] = useState("");
  const [executionEditStatus, setExecutionEditStatus] = useState("active");
  const [executionCriteriaDraft, setExecutionCriteriaDraft] = useState([]);
  const [executionEntriesDraft, setExecutionEntriesDraft] = useState([]);
  const [executionUpdateNote, setExecutionUpdateNote] = useState("");
  const [executionUpdateLoading, setExecutionUpdateLoading] = useState(false);
  const [executionUpdateError, setExecutionUpdateError] = useState("");
  const [executionUpdateSuccess, setExecutionUpdateSuccess] = useState("");

  const [distinctionTargetId, setDistinctionTargetId] = useState("");
  const [distinctionType, setDistinctionType] = useState("different_scope");
  const [distinctionText, setDistinctionText] = useState("");
  const [distinctionLoading, setDistinctionLoading] = useState(false);
  const [distinctionError, setDistinctionError] = useState("");
  const [distinctionSuccess, setDistinctionSuccess] = useState("");

  const [selectedAppeal, setSelectedAppeal] = useState(null);
  const [appealReason, setAppealReason] = useState("");
  const [appealClarification, setAppealClarification] = useState("");
  const [appealLoading, setAppealLoading] = useState(false);
  const [appealError, setAppealError] = useState("");
  const [appealSuccess, setAppealSuccess] = useState("");
  const [appealResolveNote, setAppealResolveNote] = useState("");
  const [appealResolveLoading, setAppealResolveLoading] = useState(false);
  const [appealResolveError, setAppealResolveError] = useState("");
  const [appealResolveSuccess, setAppealResolveSuccess] = useState("");

  const [selectedReconsideration, setSelectedReconsideration] = useState(null);
  const [reconsiderationReason, setReconsiderationReason] = useState("");
  const [reconsiderationNote, setReconsiderationNote] = useState("");
  const [reconsiderationLoading, setReconsiderationLoading] = useState(false);
  const [reconsiderationError, setReconsiderationError] = useState("");
  const [reconsiderationSuccess, setReconsiderationSuccess] = useState("");
  const [reconsiderationResolveNote, setReconsiderationResolveNote] = useState("");
  const [reconsiderationResolveLoading, setReconsiderationResolveLoading] = useState(false);
  const [reconsiderationResolveError, setReconsiderationResolveError] = useState("");
  const [reconsiderationResolveSuccess, setReconsiderationResolveSuccess] = useState("");

  const [unlockStatus, setUnlockStatus] = useState(null);
  const [unlockError, setUnlockError] = useState("");
  const [reviewQueues, setReviewQueues] = useState(emptyReviewQueues);
  const [issueOptions, setIssueOptions] = useState([]);
  const [solutionTargetOptions, setSolutionTargetOptions] = useState([]);
  const [solutionOptions, setSolutionOptions] = useState([]);
  const [solutionTargetIsPublishedWinner, setSolutionTargetIsPublishedWinner] =
    useState(false);
  const [outcomeData, setOutcomeData] = useState(null);
  const [outcomeResolveLoading, setOutcomeResolveLoading] = useState(false);
  const [outcomeResolveError, setOutcomeResolveError] = useState("");
  const [outcomeResolveSuccess, setOutcomeResolveSuccess] = useState("");

  const [reviewActionLoading, setReviewActionLoading] = useState(false);
  const [reviewActionError, setReviewActionError] = useState("");
  const [reviewActionSuccess, setReviewActionSuccess] = useState("");
  const [trustResolveLoading, setTrustResolveLoading] = useState(false);
  const [trustResolveError, setTrustResolveError] = useState("");
  const [trustResolveSuccess, setTrustResolveSuccess] = useState("");

  const [voteLoading, setVoteLoading] = useState(false);
  const [voteError, setVoteError] = useState("");
  const [voteSuccess, setVoteSuccess] = useState("");
  const [localSentimentVotes, setLocalSentimentVotes] = useState({});

  const [issueForm, setIssueForm] = useState(emptyIssueForm);
  const [solutionForm, setSolutionForm] = useState(emptySolutionForm);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [submitError, setSubmitError] = useState("");
  const [submitSuccess, setSubmitSuccess] = useState("");
  const [submissionPanelOpen, setSubmissionPanelOpen] = useState(false);
  const [searchPanelOpen, setSearchPanelOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const canParticipate = Boolean(me?.email_verified);

  useEffect(() => {
    initializeSession();
  }, []);

  useEffect(() => {
    if (me) {
      loadTabData(activeTab);
    }
    // loadTabData is kept as an event-style helper in this prototype.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [me, activeTab, sessionChecked]);

  const isModerator = useMemo(() => {
    return MODERATOR_ROLES.has(me?.role_code);
  }, [me]);

  const visibleTabs = useMemo(() => {
    return TABS.filter((tab) => !tab.moderatorOnly || isModerator);
  }, [isModerator]);

  async function initializeSession() {
    try {
      const data = await api.me();
      setMe(data);
    } catch {
      setMe(null);
    } finally {
      setSessionChecked(true);
    }
  }

  function clearAppStateForSignedOutUser() {
    setMe(null);
    setItems([]);
    setItemsError("");
    setSelectedProposal(null);
    setSelectedProposalError("");
    setSelectedExecution(null);
    setSelectedExecutionError("");
    setNavDrawerOpen(false);
    setDetailDrawerOpen(false);
    setModerationError("");
    setModerationSuccess("");
    setMergeError("");
    setMergeSuccess("");
    setExecutionError("");
    setExecutionSuccess("");
    setExecutionUpdateError("");
    setExecutionUpdateSuccess("");
    setExecutionUpdateNote("");
    setMergeVoteTargetId("");
    setDistinctionTargetId("");
    setDistinctionType("different_scope");
    setDistinctionText("");
    setDistinctionError("");
    setDistinctionSuccess("");
    setSelectedAppeal(null);
    setAppealError("");
    setAppealSuccess("");
    setAppealResolveError("");
    setAppealResolveSuccess("");
    setSelectedReconsideration(null);
    setReconsiderationReason("");
    setReconsiderationNote("");
    setReconsiderationError("");
    setReconsiderationSuccess("");
    setReconsiderationResolveError("");
    setReconsiderationResolveSuccess("");
    setUnlockStatus(null);
    setUnlockError("");
    setReviewQueues(emptyReviewQueues);
    setIssueOptions([]);
    setSolutionTargetOptions([]);
    setSolutionOptions([]);
    setSolutionTargetIsPublishedWinner(false);
    setOutcomeData(null);
    setOutcomeResolveError("");
    setOutcomeResolveSuccess("");
    setReviewActionError("");
    setReviewActionSuccess("");
    setVoteError("");
    setVoteSuccess("");
    setLocalSentimentVotes({});
    setSubmitError("");
    setSubmitSuccess("");
    setSubmissionPanelOpen(false);
    setSearchPanelOpen(false);
    setSearchQuery("");
    setVerificationToken("");
    setVerificationError("");
    setVerificationSuccess("");
  }

  async function handleProtectedError(error) {
    if (error?.message === "Not authenticated.") {
      clearAppStateForSignedOutUser();
      setAuthError("Your session expired. Log in again.");
      return true;
    }
    return false;
  }

  function getBoardForTab(tabKey) {
    if (tabKey === "reviewPool") return requiredReviewBoard;
    return tabKey === "solutions" ? "solution" : "issue";
  }

  function tabUsesReviewGate(tabKey) {
    return tabKey === "issues" || tabKey === "solutions" || tabKey === "reviewPool";
  }

  function getTabForBoard(boardCode) {
    return boardCode === "solution" ? "solutions" : "issues";
  }

  function handleSelectPrototypeAccount(account) {
    setAuthMode("login");
    setEmail(account.email);
    setPassword(account.password);
    setAuthError("");
    setAuthSuccess("");
    setPasswordResetToken("");
    setPasswordResetNewPassword("");
  }

  async function loadParticipationData(boardCode = getBoardForTab(activeTab)) {
    try {
      setUnlockError("");

      const [statusData, queuesData, issuesData, solutionsData, resultsData] = await Promise.all([
        api.getUnlockStatus(boardCode),
        api.getMyReviewQueues(),
        api.getIssues(),
        api.getSolutions(),
        api.getCycleResults(),
      ]);

      const publishedWinningIssue = (resultsData.results || []).find(
        (result) =>
          result.board_code === "issue" &&
          result.result_status === "resolved" &&
          result.winning_proposal
      );
      const nextSolutionTargetOptions = publishedWinningIssue?.winning_proposal
        ? [publishedWinningIssue.winning_proposal]
        : [];

      setUnlockStatus(statusData);
      setReviewQueues({
        issues_to_review: queuesData.issues_to_review || [],
        solutions_to_review: queuesData.solutions_to_review || [],
        issues_reviewed: queuesData.issues_reviewed || [],
        solutions_reviewed: queuesData.solutions_reviewed || [],
      });
      setIssueOptions(issuesData.proposals || []);
      setSolutionTargetOptions(nextSolutionTargetOptions);
      setSolutionOptions(solutionsData.proposals || []);
      setSolutionTargetIsPublishedWinner(Boolean(publishedWinningIssue));
      setSolutionForm((current) => {
        const selectedTargetStillValid =
          current.parentIssueProposalId &&
          nextSolutionTargetOptions.some(
            (issue) => issue.id === current.parentIssueProposalId
          );

        if (selectedTargetStillValid) {
          return current;
        }

        if (publishedWinningIssue?.winning_proposal) {
          return {
            ...current,
            parentIssueProposalId: publishedWinningIssue.winning_proposal.id,
          };
        }

        if (current.parentIssueProposalId) {
          return { ...current, parentIssueProposalId: "" };
        }

        return current;
      });
      return statusData;
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setUnlockError(error.message || "Failed to load participation status.");
      return null;
    }
  }

  async function loadTabData(tabKey) {
    try {
      setItemsLoading(true);
      setItemsError("");

      let data;

      if (tabKey === "issues") {
        data = await api.getIssues();
        setItems(data.proposals || []);
        setOutcomeData(null);
      } else if (tabKey === "solutions") {
        data = await api.getSolutions();
        setItems(data.proposals || []);
        setOutcomeData(null);
      } else if (tabKey === "archive") {
        data = await api.getArchive();
        setItems(data.proposals || []);
        setOutcomeData(null);
      } else if (tabKey === "reviewPool") {
        data = await api.getReviewPool(
          getBoardForTab(tabKey),
          REQUIRED_REVIEW_DISPLAY_LIMIT
        );
        setItems(data.proposals || []);
        setOutcomeData(null);
      } else if (tabKey === "implementations") {
        data = await api.getExecutionRecords();
        setItems(data.execution_records || []);
        setOutcomeData(null);
      } else if (tabKey === "outcomes" && isModerator) {
        data = await api.getCurrentCycleOutcomes();
        setItems([
          ...(data.issue_candidates || []),
          ...(data.solution_candidates || []),
        ]);
        setOutcomeData(data);
      } else if (tabKey === "reviewQueue" && isModerator) {
        data = await api.getReviewQueue();
        setItems(data.proposals || []);
        setOutcomeData(null);
      } else if (tabKey === "trustReview" && isModerator) {
        data = await api.getAntiAbuseQueue();
        setItems(data.flags || []);
        setOutcomeData(null);
      } else if (tabKey === "appeals" && isModerator) {
        data = await api.getAppealQueue();
        setItems(data.appeals || []);
        setOutcomeData(null);
      } else if (tabKey === "reconsiderations" && isModerator) {
        data = await api.getReconsiderationQueue();
        setItems(data.reconsiderations || []);
        setOutcomeData(null);
      } else {
        setItems([]);
        setOutcomeData(null);
      }

      const reviewGateAppliesForTab = tabUsesReviewGate(tabKey);
      const statusData = await loadParticipationData(
        reviewGateAppliesForTab ? getBoardForTab(tabKey) : requiredReviewBoard
      );
      const activeBoard = tabKey === "solutions" ? "solution" : "issue";
      const shouldShowRequiredReviews =
        reviewGateAppliesForTab &&
        canParticipate &&
        statusData &&
        !statusData.review_unlocked &&
        statusData.required_review_actions > statusData.completed_review_actions;

      if (tabKey === "reviewPool" && !shouldShowRequiredReviews) {
        setItems([]);
        setOutcomeData(null);
        setActiveTab(getTabForBoard(statusData?.board_code || getBoardForTab(tabKey)));
        return;
      }

      if (shouldShowRequiredReviews && tabKey !== "reviewPool") {
        const nextRequiredReviewBoard = statusData.board_code || activeBoard;
        const reviewData = await api.getReviewPool(
          nextRequiredReviewBoard,
          REQUIRED_REVIEW_DISPLAY_LIMIT
        );

        setItems(reviewData.proposals || []);
        setOutcomeData(null);
        setRequiredReviewBoard(nextRequiredReviewBoard);
        setSelectedProposal(null);
        setSelectedExecution(null);
        setSubmissionPanelOpen(false);
        setSearchPanelOpen(false);
        setNavDrawerOpen(false);
        setDetailDrawerOpen(false);
        setActiveTab("reviewPool");
      }
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setItemsError(error.message || "Failed to load data.");
    } finally {
      setItemsLoading(false);
    }
  }

  async function handleAuthSubmit(event) {
    event.preventDefault();

    try {
      setAuthLoading(true);
      setAuthError("");
      setAuthSuccess("");
      setVerificationToken("");
      setVerificationError("");
      setVerificationSuccess("");

      if (authMode === "login") {
        const loginData = await api.login(email, password);
        setMe(loginData);
        handleTabChange("issues");
      } else if (authMode === "register") {
        const registerData = await api.register(email, password);
        const loginData = await api.login(email, password);
        setMe(loginData);
        handleTabChange("issues");
        if (registerData.dev_verification_token) {
          setVerificationToken(registerData.dev_verification_token);
          setVerificationSuccess("Verification token generated for this prototype.");
        } else if (registerData.verification_email_sent === false) {
          setVerificationError(
            "Account created, but the verification email could not be sent. Try requesting another token in a moment."
          );
        } else if (registerData.verification_required) {
          setVerificationSuccess("Verification instructions sent.");
        }
      } else if (authMode === "resetRequest") {
        const resetData = await api.requestPasswordReset(email);
        if (resetData.dev_reset_token) {
          setPasswordResetToken(resetData.dev_reset_token);
          setAuthMode("resetConfirm");
          setAuthSuccess("Reset token generated for this prototype.");
        } else {
          setAuthMode("resetConfirm");
          setAuthSuccess("If that account exists, reset instructions were sent.");
        }
      } else if (authMode === "resetConfirm") {
        await api.confirmPasswordReset(passwordResetToken, passwordResetNewPassword);
        setPassword("");
        setPasswordResetToken("");
        setPasswordResetNewPassword("");
        setAuthMode("login");
        setAuthSuccess("Password reset. Log in with your new password.");
      }

      setSessionChecked(true);
    } catch (error) {
      setAuthError(error.message || "Authentication failed.");
    } finally {
      setAuthLoading(false);
    }
  }

  async function handleVerifyEmail(event) {
    event.preventDefault();

    try {
      setVerificationLoading(true);
      setVerificationError("");
      setVerificationSuccess("");

      await api.verifyEmail(verificationToken);
      const data = await api.me();
      setMe(data);
      setVerificationToken("");
      setVerificationSuccess("Email verified.");
      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setVerificationError(error.message || "Failed to verify email.");
    } finally {
      setVerificationLoading(false);
    }
  }

  async function handleRequestVerificationToken() {
    try {
      setVerificationLoading(true);
      setVerificationError("");
      setVerificationSuccess("");

      const data = await api.requestEmailVerificationToken();
      if (data.dev_verification_token) {
        setVerificationToken(data.dev_verification_token);
      }
      if (data.email_verified) {
        setVerificationSuccess("Email already verified.");
      } else if (data.dev_verification_token) {
        setVerificationSuccess("Verification token generated for this prototype.");
      } else if (data.verification_email_sent === false) {
        setVerificationError(
          "Verification email could not be sent. Try again in a moment."
        );
      } else {
        setVerificationSuccess("Verification instructions sent.");
      }
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setVerificationError(error.message || "Failed to request verification.");
    } finally {
      setVerificationLoading(false);
    }
  }

  async function handleSelectProposal(id) {
    try {
      setSelectedProposalLoading(true);
      setSelectedProposalError("");
      setSelectedExecution(null);
      setSelectedExecutionError("");
      setModerationError("");
      setModerationSuccess("");
      setMergeError("");
      setMergeSuccess("");
      setExecutionError("");
      setExecutionSuccess("");
      setExecutionUpdateError("");
      setExecutionUpdateSuccess("");
      setDistinctionError("");
      setDistinctionSuccess("");
      setAppealError("");
      setAppealSuccess("");
      setAppealResolveError("");
      setAppealResolveSuccess("");
      setReconsiderationError("");
      setReconsiderationSuccess("");
      setReconsiderationResolveError("");
      setReconsiderationResolveSuccess("");
      setDetailDrawerOpen(true);
      setFrontDrawer("detail");

      const data = await api.getProposal(id);
      const localItem = items.find((item) => getItemId(item) === id);
      setSelectedAppeal(activeTab === "appeals" && localItem?.appeal_id ? localItem : null);
      setSelectedReconsideration(
        activeTab === "reconsiderations" && localItem?.reconsideration_id
          ? localItem
          : null
      );
      const localProposal = localItem?.proposal || localItem || {};
      const mergedData =
        isModerator && localProposal
          ? {
              ...data,
              merge_relationships:
                localItem?.merge_relationships || data.merge_relationships,
              proposal: {
                ...data.proposal,
                ...localProposal,
                review_reason: localItem?.review_reason || localProposal.review_reason,
              },
            }
          : data;

      setSelectedProposal(mergedData);
      setModerationNote(data?.proposal?.moderation_note || "");

      const firstIncomingTarget =
        data?.merge_relationships?.incoming?.[0]?.source_proposal_id || "";
      const firstOutgoingTarget =
        data?.merge_relationships?.outgoing?.[0]?.target_proposal_id || "";

      setMergeTargetId(firstOutgoingTarget || firstIncomingTarget || "");
      seedDistinctionForm(mergedData);
      const participationBoard =
        activeTab === "solutions" && mergedData?.proposal?.board_code === "issue"
          ? "solution"
          : mergedData?.proposal?.board_code || getBoardForTab(activeTab);
      await loadParticipationData(participationBoard);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setSelectedProposalError(error.message || "Failed to load proposal.");
      setSelectedProposal(null);
      setMergeTargetId("");
      setMergeVoteTargetId("");
      resetDistinctionForm();
      setSelectedReconsideration(null);
    } finally {
      setSelectedProposalLoading(false);
    }
  }

  async function handleSelectExecution(id) {
    try {
      setSelectedExecutionLoading(true);
      setSelectedExecutionError("");
      setSelectedProposal(null);
      setSelectedProposalError("");
      setDetailDrawerOpen(true);
      setFrontDrawer("detail");

      const data = await api.getExecutionRecord(id);
      setSelectedExecution(data.execution_record);
      seedExecutionUpdateForm(data.execution_record);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setSelectedExecutionError(error.message || "Failed to load implementation record.");
      setSelectedExecution(null);
    } finally {
      setSelectedExecutionLoading(false);
    }
  }

  async function handleModerateArchive() {
    if (!selectedProposal?.proposal?.id) return;

    try {
      setModerationLoading(true);
      setModerationError("");
      setModerationSuccess("");

      await api.moderateArchive(
        selectedProposal.proposal.id,
        moderationReason,
        moderationNote
      );

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal(refreshed);
      setModerationSuccess("Proposal archived successfully.");

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setModerationError(error.message || "Failed to archive proposal.");
    } finally {
      setModerationLoading(false);
    }
  }

  async function handleModerateFreeze() {
    if (!selectedProposal?.proposal?.id) return;

    try {
      setModerationLoading(true);
      setModerationError("");
      setModerationSuccess("");

      await api.moderateFreeze(selectedProposal.proposal.id, moderationNote);

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal({
        ...refreshed,
        proposal: {
          ...refreshed.proposal,
          review_reason: "frozen_review",
        },
      });
      setModerationSuccess("Proposal frozen for review.");

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setModerationError(error.message || "Failed to freeze proposal.");
    } finally {
      setModerationLoading(false);
    }
  }

  async function handleModerateUnfreeze() {
    if (!selectedProposal?.proposal?.id) return;

    try {
      setModerationLoading(true);
      setModerationError("");
      setModerationSuccess("");

      await api.moderateUnfreeze(selectedProposal.proposal.id, moderationNote);

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal(refreshed);
      setModerationSuccess("Proposal returned to active review.");

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setModerationError(error.message || "Failed to unfreeze proposal.");
    } finally {
      setModerationLoading(false);
    }
  }

  async function handleModerateReviewedActive() {
    if (!selectedProposal?.proposal?.id) return;

    try {
      setModerationLoading(true);
      setModerationError("");
      setModerationSuccess("");

      await api.moderateReviewedActive(selectedProposal.proposal.id, moderationNote);

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal(refreshed);
      setModerationSuccess("Proposal marked reviewed and left active.");

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setModerationError(error.message || "Failed to mark proposal reviewed.");
    } finally {
      setModerationLoading(false);
    }
  }

  async function handleExecuteMerge() {
    if (!selectedProposal?.proposal?.id || !mergeTargetId) return;

    try {
      setMergeLoading(true);
      setMergeError("");
      setMergeSuccess("");

      const result = await api.executeMerge(selectedProposal.proposal.id, mergeTargetId);

      const detailId =
        result.archived_proposal_id === selectedProposal.proposal.id
          ? result.surviving_proposal_id
          : selectedProposal.proposal.id;
      const refreshed = await api.getProposal(detailId);
      setSelectedProposal(refreshed);
      setMergeSuccess(
        `Merged lower-count proposal into higher-count proposal. Transferred ${result.sentiment_votes_transferred} sentiment vote(s); discarded ${result.sentiment_votes_discarded_conflicting} conflicting vote(s).`
      );

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setMergeError(error.message || "Failed to execute merge.");
    } finally {
      setMergeLoading(false);
    }
  }

  async function handleCreateExecutionRecord() {
    if (!selectedProposal?.proposal?.id) return;

    try {
      setExecutionLoading(true);
      setExecutionError("");
      setExecutionSuccess("");

      const data = await api.createExecutionRecord(selectedProposal.proposal.id);
      setSelectedExecution(data.execution_record);
      seedExecutionUpdateForm(data.execution_record);
      setSelectedProposal(null);
      setExecutionSuccess("Implementation record created.");

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setExecutionError(error.message || "Failed to create implementation record.");
    } finally {
      setExecutionLoading(false);
    }
  }

  async function handleResolveCurrentCycle() {
    try {
      setOutcomeResolveLoading(true);
      setOutcomeResolveError("");
      setOutcomeResolveSuccess("");

      const data = await api.resolveCurrentCycleOutcomes();
      const archivedText = `${data.archived_proposal_count || 0} proposal(s) archived`;
      setOutcomeResolveSuccess(
        data.execution_record_id
          ? `Cycle resolved, implementation record created, ${archivedText}, and next cycle opened.`
          : `Cycle resolved, ${archivedText}, and next cycle opened.`
      );
      await loadTabData("outcomes");
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setOutcomeResolveError(error.message || "Failed to resolve cycle.");
    } finally {
      setOutcomeResolveLoading(false);
    }
  }

  function seedExecutionUpdateForm(record) {
    setExecutionEditStatus(record?.status || "active");
    setExecutionCriteriaDraft(
      asArray(record?.completion_criteria).map((criterion) =>
        typeof criterion === "string"
          ? {
              criterion_description: criterion,
              completion_status: "not_started",
              evidence_note: "",
              updated_at: null,
            }
          : { ...criterion }
      )
    );
    setExecutionEntriesDraft(
      asArray(record?.execution_tracking_entries).map((entry) =>
        typeof entry === "string"
          ? {
              resource_category: "other",
              target_needed: entry,
              current_acquired_amount: "",
              external_coordination_link: "",
              status_proof_note: "",
            }
          : { ...entry }
      )
    );
    setExecutionUpdateNote("");
  }

  function updateExecutionCriterion(index, field, value) {
    setExecutionCriteriaDraft((current) =>
      current.map((criterion, currentIndex) =>
        currentIndex === index
          ? {
              ...criterion,
              [field]: value,
              updated_at: new Date().toISOString(),
            }
          : criterion
      )
    );
  }

  function updateExecutionEntry(index, field, value) {
    setExecutionEntriesDraft((current) =>
      current.map((entry, currentIndex) =>
        currentIndex === index
          ? {
              ...entry,
              [field]: value,
            }
          : entry
      )
    );
  }

  async function handleSaveExecutionUpdate(event) {
    event.preventDefault();

    if (!selectedExecution?.id) return;

    try {
      setExecutionUpdateLoading(true);
      setExecutionUpdateError("");
      setExecutionUpdateSuccess("");

      const data = await api.updateExecutionRecord(selectedExecution.id, {
        status: executionEditStatus,
        completion_criteria: executionCriteriaDraft,
        execution_tracking_entries: executionEntriesDraft,
        update_note: executionUpdateNote,
      });

      setSelectedExecution(data.execution_record);
      seedExecutionUpdateForm(data.execution_record);
      setExecutionUpdateSuccess("Implementation record updated.");
      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setExecutionUpdateError(error.message || "Failed to update implementation record.");
    } finally {
      setExecutionUpdateLoading(false);
    }
  }

  function resetDistinctionForm() {
    setDistinctionTargetId("");
    setDistinctionType("different_scope");
    setDistinctionText("");
  }

  function seedDistinctionForm(detailData) {
    const outgoing = detailData?.merge_relationships?.outgoing || [];
    const firstOutgoing = outgoing[0];

    if (!firstOutgoing) {
      resetDistinctionForm();
      return;
    }

    setDistinctionTargetId(firstOutgoing.target_proposal_id);
    setDistinctionType(firstOutgoing.note?.difference_type || "different_scope");
    setDistinctionText(firstOutgoing.note?.note_text || "");
  }

  function handleDistinctionTargetChange(targetId) {
    const relationship = selectedOutgoingRelationships.find(
      (rel) => rel.target_proposal_id === targetId
    );

    setDistinctionTargetId(targetId);
    setDistinctionType(relationship?.note?.difference_type || "different_scope");
    setDistinctionText(relationship?.note?.note_text || "");
    setDistinctionError("");
    setDistinctionSuccess("");
  }

  async function handleSaveDistinctionNote(event) {
    event.preventDefault();

    if (!selectedProposal?.proposal?.id || !distinctionTargetId) return;

    try {
      setDistinctionLoading(true);
      setDistinctionError("");
      setDistinctionSuccess("");

      await api.upsertMergeDistinctionNote(
        selectedProposal.proposal.id,
        distinctionTargetId,
        distinctionType,
        distinctionText
      );

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal(refreshed);
      seedDistinctionForm(refreshed);
      setDistinctionSuccess("Distinction note saved.");
      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setDistinctionError(error.message || "Failed to save distinction note.");
    } finally {
      setDistinctionLoading(false);
    }
  }

  async function handleSubmitAppeal(event) {
    event.preventDefault();

    if (!selectedProposal?.proposal?.id) return;

    try {
      setAppealLoading(true);
      setAppealError("");
      setAppealSuccess("");

      await api.submitAppeal(
        selectedProposal.proposal.id,
        appealReason,
        appealClarification
      );

      setAppealReason("");
      setAppealClarification("");
      setAppealSuccess("Appeal submitted.");

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal(refreshed);
      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setAppealError(error.message || "Failed to submit appeal.");
    } finally {
      setAppealLoading(false);
    }
  }

  async function handleResolveAppeal(outcome) {
    if (!selectedAppeal?.appeal_id) return;

    try {
      setAppealResolveLoading(true);
      setAppealResolveError("");
      setAppealResolveSuccess("");

      const result = await api.resolveAppeal(
        selectedAppeal.appeal_id,
        outcome,
        appealResolveNote
      );

      const refreshed = await api.getProposal(result.proposal_id);
      setSelectedProposal(refreshed);
      setSelectedAppeal((current) =>
        current
          ? {
              ...current,
              status: result.appeal_status,
              outcome: result.outcome,
            }
          : current
      );
      setAppealResolveNote("");
      setAppealResolveSuccess(
        outcome === "restore" ? "Proposal restored." : "Archive upheld."
      );

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setAppealResolveError(error.message || "Failed to resolve appeal.");
    } finally {
      setAppealResolveLoading(false);
    }
  }

  async function handleStartReconsideration(event) {
    event.preventDefault();

    if (!selectedProposal?.proposal?.id) return;

    try {
      setReconsiderationLoading(true);
      setReconsiderationError("");
      setReconsiderationSuccess("");

      await api.startReconsideration(
        selectedProposal.proposal.id,
        reconsiderationReason,
        reconsiderationNote
      );

      setReconsiderationReason("");
      setReconsiderationNote("");
      setReconsiderationSuccess("Reconsideration window opened.");

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal(refreshed);
      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setReconsiderationError(error.message || "Failed to start reconsideration.");
    } finally {
      setReconsiderationLoading(false);
    }
  }

  async function handleResolveReconsideration(outcome) {
    if (!selectedReconsideration?.reconsideration_id) return;

    try {
      setReconsiderationResolveLoading(true);
      setReconsiderationResolveError("");
      setReconsiderationResolveSuccess("");

      const result = await api.resolveReconsideration(
        selectedReconsideration.reconsideration_id,
        outcome,
        reconsiderationResolveNote
      );

      const refreshed = await api.getProposal(result.proposal_id);
      setSelectedProposal(refreshed);
      setSelectedReconsideration((current) =>
        current
          ? {
              ...current,
              status: "resolved",
              outcome: result.outcome,
              primary_state: result.proposal_primary_state,
            }
          : current
      );
      setReconsiderationResolveNote("");
      setReconsiderationResolveSuccess("Reconsideration resolved.");

      await loadTabData(activeTab);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setReconsiderationResolveError(
        error.message || "Failed to resolve reconsideration."
      );
    } finally {
      setReconsiderationResolveLoading(false);
    }
  }

  async function handleSubmitReviewAction(proposalId, voteValue) {
    if (!proposalId || !voteValue) return;
    const reviewedBoard =
      activeTab === "reviewPool"
        ? requiredReviewBoard
        : selectedProposal?.proposal?.board_code || getBoardForTab(activeTab);

    try {
      setReviewActionLoading(true);
      setReviewActionError("");
      setReviewActionSuccess("");

      const result = await api.submitReviewAction(proposalId, voteValue);
      setLocalSentimentVotes((current) => ({
        ...current,
        [proposalId]: result.sentiment_vote || voteValue,
      }));
      setReviewActionSuccess("Review vote credited.");

      if (selectedProposal?.proposal?.id === proposalId) {
        const refreshed = await api.getProposal(proposalId);
        setSelectedProposal(refreshed);
      }

      await loadParticipationData(reviewedBoard);
      if (result.review_unlocked) {
        handleTabChange(getTabForBoard(reviewedBoard));
      } else {
        await loadTabData(activeTab);
      }
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setReviewActionError(error.message || "Failed to save review vote.");
    } finally {
      setReviewActionLoading(false);
    }
  }

  async function handleResolveTrustFlag(flagId, outcome) {
    if (!flagId || !outcome) return;

    const resolutionNote =
      outcome === "dismissed"
        ? "Dismissed from the Trust Review queue."
        : "Acknowledged from the Trust Review queue.";

    try {
      setTrustResolveLoading(true);
      setTrustResolveError("");
      setTrustResolveSuccess("");

      await api.resolveAntiAbuseFlag(flagId, outcome, resolutionNote);
      setTrustResolveSuccess(
        outcome === "dismissed" ? "Flag dismissed." : "Flag acknowledged."
      );
      await loadTabData("trustReview");
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setTrustResolveError(error.message || "Failed to resolve trust review flag.");
    } finally {
      setTrustResolveLoading(false);
    }
  }

  async function handleSentimentVote(voteValue) {
    if (!selectedProposal?.proposal?.id) return;
    const proposalId = selectedProposal.proposal.id;

    try {
      setVoteLoading(true);
      setVoteError("");
      setVoteSuccess("");

      const result = await api.castSentimentVote(proposalId, voteValue);
      setLocalSentimentVotes((current) => ({
        ...current,
        [proposalId]: result.sentiment_vote || voteValue,
      }));
      setVoteSuccess("Vote saved.");

      const refreshed = await api.getProposal(proposalId);
      setSelectedProposal(refreshed);
      await Promise.all([
        loadParticipationData(refreshed.proposal.board_code),
        loadTabData(activeTab),
      ]);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setVoteError(error.message || "Failed to save vote.");
    } finally {
      setVoteLoading(false);
    }
  }

  async function handleMergeVote() {
    if (!selectedProposal?.proposal?.id || !currentMergeVoteTargetId) return;

    const targetId = currentMergeVoteTargetId.trim();
    if (!isUuid(targetId)) {
      setVoteSuccess("");
      setVoteError("Enter a valid submission ID.");
      return;
    }

    if (targetId === selectedProposal.proposal.id) {
      setVoteSuccess("");
      setVoteError("Use the ID from a different active submission.");
      return;
    }

    try {
      setVoteLoading(true);
      setVoteError("");
      setVoteSuccess("");

      await api.castMergeVote(
        selectedProposal.proposal.id,
        targetId
      );
      setVoteSuccess("Duplicate link saved.");

      const refreshed = await api.getProposal(selectedProposal.proposal.id);
      setSelectedProposal(refreshed);
      seedDistinctionForm(refreshed);
      await Promise.all([
        loadParticipationData(refreshed.proposal.board_code),
        loadTabData(activeTab),
      ]);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setVoteError(error.message || "Failed to save merge signal.");
    } finally {
      setVoteLoading(false);
    }
  }

  function updateIssueForm(field, value) {
    setIssueForm((current) => ({ ...current, [field]: value }));
  }

  function updateSolutionForm(field, value) {
    setSolutionForm((current) => ({ ...current, [field]: value }));
  }

  function updateSolutionCriterion(index, field, value) {
    setSolutionForm((current) => ({
      ...current,
      completionCriteria: asArray(current.completionCriteria).map((criterion, itemIndex) =>
        itemIndex === index ? { ...criterion, [field]: value } : criterion
      ),
    }));
  }

  function addSolutionCriterion() {
    setSolutionForm((current) => {
      const currentCriteria = asArray(current.completionCriteria);

      if (currentCriteria.length >= MAX_COMPLETION_CRITERIA) {
        return current;
      }

      return {
        ...current,
        completionCriteria: [
          ...currentCriteria,
          createEmptyCompletionCriterion(),
        ],
      };
    });
  }

  function removeSolutionCriterion(index) {
    setSolutionForm((current) => {
      const nextCriteria = asArray(current.completionCriteria).filter(
        (_, itemIndex) => itemIndex !== index
      );

      return {
        ...current,
        completionCriteria: nextCriteria.length
          ? nextCriteria
          : [createEmptyCompletionCriterion()],
      };
    });
  }

  function updateSolutionExecutionEntry(index, field, value) {
    setSolutionForm((current) => ({
      ...current,
      executionTrackingEntries: asArray(current.executionTrackingEntries).map(
        (entry, itemIndex) =>
          itemIndex === index ? { ...entry, [field]: value } : entry
      ),
    }));
  }

  function addSolutionExecutionEntry() {
    setSolutionForm((current) => {
      const currentEntries = asArray(current.executionTrackingEntries);

      if (currentEntries.length >= MAX_RESOURCE_REQUIREMENTS) {
        return current;
      }

      return {
        ...current,
        executionTrackingEntries: [
          ...currentEntries,
          createEmptyExecutionEntry(),
        ],
      };
    });
  }

  function removeSolutionExecutionEntry(index) {
    setSolutionForm((current) => {
      const nextEntries = asArray(current.executionTrackingEntries).filter(
        (_, itemIndex) => itemIndex !== index
      );

      return {
        ...current,
        executionTrackingEntries: nextEntries.length
          ? nextEntries
          : [createEmptyExecutionEntry()],
      };
    });
  }

  function handleUseArchivedProposalAsDraft() {
    const proposal = selectedProposal?.proposal;
    if (!proposal) return;

    setSubmitError("");

    if (proposal.board_code === "issue") {
      setIssueForm({
        title: proposal.title || "",
        problemDescription: proposal.problem_description || "",
        affectedScope: proposal.affected_scope || "",
        whyItMatters: proposal.why_it_matters || "",
      });
      handleTabChange("issues");
      setSubmissionPanelOpen(true);
      setSubmitSuccess("Issue draft filled from archive. Review before submitting.");
      return;
    }

    if (proposal.board_code === "solution") {
      const currentTarget =
        solutionTargetIsPublishedWinner && solutionTargetOptions.length === 1
          ? solutionTargetOptions[0].id
          : solutionTargetOptions.some(
                (issue) => issue.id === proposal.parent_issue_proposal_id
              )
            ? proposal.parent_issue_proposal_id
            : solutionForm.parentIssueProposalId;

      setSolutionForm({
        title: proposal.title || "",
        parentIssueProposalId: currentTarget || "",
        actionDescription: proposal.action_description || "",
        whyThisSolvesIt: proposal.why_it_matters || "",
        completionCriteria: serializeCompletionCriteria(proposal.completion_criteria),
        executionTrackingEntries: serializeExecutionEntries(
          proposal.execution_tracking_entries
        ),
      });
      handleTabChange("solutions");
      setSubmissionPanelOpen(true);
      setSubmitSuccess(
        "Solution draft filled from archive. Review the current issue target before submitting."
      );
    }
  }

  async function handleSubmitIssue(event) {
    event.preventDefault();

    try {
      setSubmitLoading(true);
      setSubmitError("");
      setSubmitSuccess("");

      const result = await api.createProposal({
        board_code: "issue",
        title: issueForm.title,
        problem_description: issueForm.problemDescription,
        affected_scope: issueForm.affectedScope,
        why_it_matters: issueForm.whyItMatters,
      });

      setIssueForm(emptyIssueForm);
      setSubmitSuccess("Issue submitted.");
      setSubmissionPanelOpen(false);
      await Promise.all([loadTabData("issues"), loadParticipationData("issue")]);
      setActiveTab("issues");
      await handleSelectProposal(result.proposal_id);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setSubmitError(error.message || "Failed to submit issue.");
    } finally {
      setSubmitLoading(false);
    }
  }

  async function handleSubmitSolution(event) {
    event.preventDefault();
    const parentIssueProposalId =
      solutionTargetProposal?.id || solutionForm.parentIssueProposalId;

    try {
      setSubmitLoading(true);
      setSubmitError("");
      setSubmitSuccess("");

      const result = await api.createProposal({
        board_code: "solution",
        title: solutionForm.title,
        action_description: solutionForm.actionDescription,
        why_it_matters: solutionForm.whyThisSolvesIt,
        parent_issue_proposal_id: parentIssueProposalId,
        required_resource_categories: parseResourceCategories(
          solutionForm.executionTrackingEntries
        ),
        completion_criteria: parseCompletionCriteria(solutionForm.completionCriteria),
        execution_tracking_entries: parseExecutionEntries(
          solutionForm.executionTrackingEntries
        ),
      });

      setSolutionForm(emptySolutionForm);
      setSubmitSuccess("Solution submitted.");
      setSubmissionPanelOpen(false);
      await Promise.all([loadTabData("solutions"), loadParticipationData("solution")]);
      setActiveTab("solutions");
      await handleSelectProposal(result.proposal_id);
    } catch (error) {
      if (await handleProtectedError(error)) return;
      setSubmitError(error.message || "Failed to submit solution.");
    } finally {
      setSubmitLoading(false);
    }
  }

  async function handleLogout() {
    try {
      await api.logout();
    } catch {
      // ignore and still clear local state
    } finally {
      clearAppStateForSignedOutUser();
      setSessionChecked(true);
    }
  }

  function handleTabChange(tabKey) {
    if (tabKey === "issues") {
      setRequiredReviewBoard("issue");
    } else if (tabKey === "solutions") {
      setRequiredReviewBoard("solution");
    }

    setActiveTab(tabKey);
    setSelectedProposal(null);
    setSelectedProposalError("");
    setSelectedExecution(null);
    setSelectedExecutionError("");
    setModerationError("");
    setModerationSuccess("");
    setMergeError("");
    setMergeSuccess("");
    setExecutionError("");
    setExecutionSuccess("");
    setMergeVoteTargetId("");
    resetDistinctionForm();
    setDistinctionError("");
    setDistinctionSuccess("");
    setSelectedAppeal(null);
    setAppealError("");
    setAppealSuccess("");
    setAppealResolveError("");
    setAppealResolveSuccess("");
    setSelectedReconsideration(null);
    setReconsiderationError("");
    setReconsiderationSuccess("");
    setReconsiderationResolveError("");
    setReconsiderationResolveSuccess("");
    setOutcomeResolveError("");
    setOutcomeResolveSuccess("");
    setSubmissionPanelOpen(false);
    setSearchPanelOpen(false);
    setSearchQuery("");
    setDetailDrawerOpen(false);
  }

  function handleSubmissionButton() {
    if (currentUserSubmission?.id) {
      handleSelectProposal(currentUserSubmission.id);
      setNavDrawerOpen(false);
      setSubmissionPanelOpen(false);
      return;
    }

    setSubmitError("");
    setSubmitSuccess("");
    setSearchPanelOpen(false);
    setSelectedProposal(null);
    setSelectedExecution(null);
    setDetailDrawerOpen(false);
    setSubmissionPanelOpen(true);
    setNavDrawerOpen(false);
  }

  function toggleNavDrawer() {
    if (navigationTabs.length === 0) return;

    const shouldOpenNav = !(navDrawerOpen && frontDrawer === "nav");
    setSubmissionPanelOpen(false);
    setDetailDrawerOpen(false);
    setNavDrawerOpen(shouldOpenNav);

    if (shouldOpenNav) {
      setFrontDrawer("nav");
    }
  }

  function toggleDetailDrawer() {
    const shouldOpenDetail = !(detailDrawerOpen && frontDrawer === "detail");
    setSubmissionPanelOpen(false);
    setNavDrawerOpen(false);
    setDetailDrawerOpen(shouldOpenDetail);

    if (shouldOpenDetail) {
      setFrontDrawer("detail");
    }
  }

  const selectedTitle = useMemo(() => {
    return selectedProposal?.proposal?.title || selectedExecution?.title || "Details";
  }, [selectedProposal, selectedExecution]);

  const pageTitle = useMemo(() => {
    if (activeTab === "issues" || activeTab === "solutions") {
      return "Review Feed";
    }
    if (activeTab === "reviewPool") {
      return `Required ${formatActionType(requiredReviewBoard)} Reviews`;
    }
    const found = visibleTabs.find((tab) => tab.key === activeTab);
    return found ? found.label : "Board";
  }, [activeTab, requiredReviewBoard, visibleTabs]);

  function getItemTitle(item) {
    if (item?.flag_code) {
      return `${formatActionType(item.flag_code)}: ${item.user_email || "unknown user"}`;
    }
    if (item?.result_status) {
      const boardLabel = formatActionType(item.board_code);
      const winnerTitle = item.winning_proposal?.title || "No ranked winner";
      return `Cycle ${item.cycle_number} ${boardLabel}: ${winnerTitle}`;
    }
    if (item?.solution_proposal_id) return item.title;
    if (item?.proposal_title) return `Appeal: ${item.proposal_title}`;
    if (item?.reconsideration_id) return `Reconsider: ${item.proposal_title}`;
    if (item?.proposal?.title) return item.proposal.title;
    return item?.title || "Untitled Proposal";
  }

  function getItemId(item) {
    if (item?.flag_code) return item.id;
    if (item?.result_status) return item.winning_proposal_id || item.id;
    if (item?.solution_proposal_id) return item.id;
    if (item?.reconsideration_id && item?.proposal_id) return item.proposal_id;
    if (item?.appeal_id && item?.proposal_id) return item.proposal_id;
    if (item?.proposal?.id) return item.proposal.id;
    return item?.id;
  }

  function getItemState(item) {
    if (item?.flag_code) return item.status;
    if (item?.result_status) return item.result_status;
    if (item?.solution_proposal_id) return item.status;
    if (item?.reconsideration_id) return item.status;
    if (item?.appeal_id) return item.status;
    if (item?.proposal?.primary_state) return item.proposal.primary_state;
    return item?.primary_state;
  }

  function getPublicStateLabel(item) {
    if (item?.result_status) return formatActionType(item.result_status);

    const source = item?.proposal || item;

    if (source?.solution_proposal_id) return formatActionType(source.status);
    if (source?.is_archived || source?.primary_state === "archived") return "Archived";
    if (source?.merged_into_proposal_id) return "Merged";
    return null;
  }

  function getItemDescription(item) {
    if (item?.flag_code) {
      return (
        item.details?.summary ||
        item.proposal_title ||
        item.related_proposal_title ||
        formatActionType(item.flag_code)
      );
    }
    if (item?.result_status) {
      const source = item.winning_proposal || {};
      return (
        source.problem_description ||
        source.action_description ||
        item.result_status
      );
    }
    if (item?.solution_proposal_id) return item.action_description;
    if (item?.start_reason) return item.start_reason;
    if (item?.appeal_reason) return item.appeal_reason;
    const source = item?.proposal || item;
    return (
      source?.problem_description ||
      source?.action_description ||
      source?.why_it_matters ||
      source?.moderation_note ||
      ""
    );
  }

  function getItemCounts(item) {
    const source = item?.winning_proposal || item?.proposal || item;
    return {
      support: source?.support_count ?? 0,
      notFit: source?.not_a_fit_count ?? 0,
      unclear: source?.unclear_count ?? 0,
      unsafe: source?.unsafe_count ?? 0,
      merge: source?.merge_count ?? 0,
    };
  }

  function getTotalVoteCount(item) {
    const counts = getItemCounts(item);
    return counts.support + counts.notFit + counts.unclear + counts.unsafe + counts.merge;
  }

  function findCurrentListProposal(proposalId) {
    return items
      .map((item) => item?.proposal || item)
      .find((item) => item?.id === proposalId);
  }

  function findCurrentUserSubmission(boardCode) {
    return items
      .map((item) => item?.proposal || item)
      .find(
        (item) =>
          item?.author_user_id === me?.user_id &&
          item?.board_code === boardCode &&
          !item?.is_archived &&
          item?.primary_state !== "archived"
      );
  }

  function itemMatchesSearch(item, query) {
    if (!query.trim()) return true;

    const source = item?.winning_proposal || item?.proposal || item;
    const haystack = [
      getItemTitle(item),
      getItemDescription(item),
      source?.affected_scope,
      source?.why_it_matters,
      source?.required_resource_categories,
      source?.completion_criteria,
      source?.execution_tracking_entries,
    ]
      .map((value) =>
        typeof value === "string" ? value : value ? JSON.stringify(value) : ""
      )
      .join(" ")
      .toLowerCase();

    return haystack.includes(query.trim().toLowerCase());
  }

  function getExtraBadge(item) {
    if (!isModerator) return null;
    if (item?.result_status) {
      return `${formatActionType(item.board_code)} ${formatActionType(
        item.result_status
      )}`;
    }
    if (item?.classification) {
      const boardLabel = item?.proposal?.board_code || item?.board_code || "";
      const rankLabel = item.rank ? ` #${item.rank}` : "";
      return `${formatActionType(boardLabel)} ${formatActionType(
        item.classification
      )}${rankLabel}`;
    }
    if (item?.solution_proposal_id) return "Implementation";
    if (item?.flag_code) return formatActionType(item.severity || "review");
    if (item?.reconsideration_id) return item.review_due ? "Review Due" : "Window Open";
    if (item?.appeal_id) return "Appeal";
    if (item?.review_bucket) return item.review_bucket;
    if (item?.review_reason) return item.review_reason;
    if (item?.archived_reason) return item.archived_reason;
    return null;
  }

  function formatActionType(value) {
    return String(value || "")
      .split("_")
      .filter(Boolean)
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(" ");
  }

  function asArray(value) {
    return Array.isArray(value) ? value : [];
  }

  function formatResourceCategory(value) {
    const normalized = String(value || "").trim().toLowerCase();
    return (
      RESOURCE_CATEGORY_LABELS[normalized] ||
      formatActionType(normalized.replace(/\//g, "_"))
    );
  }

  function getCriterionDescription(item) {
    if (typeof item === "string") return item;
    return item?.criterion_description || item?.description || "Completion criterion";
  }

  function getCriterionStatus(item) {
    if (!item || typeof item === "string") return null;
    return item.completion_status || item.status || null;
  }

  function getCriterionEvidence(item) {
    if (!item || typeof item === "string") return null;
    return item.evidence_note || item.proof_note || null;
  }

  function getExecutionNote(item) {
    return item?.status_proof_note || item?.status_note || item?.proof_note || "";
  }

  function buildTargetNeeded(entry) {
    const amount = String(entry?.target_amount || "").trim();
    const unit = String(entry?.target_unit || "").trim();
    const directTarget = String(entry?.target_needed || "").trim();
    const builtTarget = [amount, unit].filter(Boolean).join(" ").trim();

    if (builtTarget.length >= 5) return builtTarget;
    if (builtTarget) {
      return [builtTarget, formatResourceCategory(entry?.resource_category || "other")]
        .filter(Boolean)
        .join(" ")
        .trim();
    }

    return directTarget;
  }

  function getRequiredResourceCategories(entries) {
    const categories = asArray(entries)
      .map((entry) => String(entry?.resource_category || "").trim())
      .filter(Boolean);

    return [...new Set(categories)].length ? [...new Set(categories)] : ["other"];
  }

  function normalizeCompletionCriterion(item) {
    if (typeof item === "string") {
      return createEmptyCompletionCriterion(item);
    }

    return {
      criterion_description:
        item?.criterion_description || item?.description || "",
      completion_status: item?.completion_status || item?.status || "not_started",
      evidence_note: item?.evidence_note || item?.proof_note || "",
      updated_at: item?.updated_at || null,
    };
  }

  function normalizeExecutionEntry(item) {
    if (typeof item === "string") {
      return createEmptyExecutionEntry({ target_needed: item, resource_category: "other" });
    }

    return createEmptyExecutionEntry({
      resource_category: item?.resource_category || "other",
      target_needed: item?.target_needed || "",
      target_amount: item?.target_amount || item?.amount_required || item?.target_needed || "",
      target_unit: item?.target_unit || item?.unit || "",
      current_acquired_amount: item?.current_acquired_amount || "",
      external_coordination_link: item?.external_coordination_link || "",
      status_proof_note: getExecutionNote(item),
    });
  }

  function serializeCompletionCriteria(value) {
    const criteria = asArray(value).map(normalizeCompletionCriterion);
    return criteria.length ? criteria : [createEmptyCompletionCriterion()];
  }

  function serializeExecutionEntries(value) {
    const entries = asArray(value).map(normalizeExecutionEntry);
    return entries.length ? entries : [createEmptyExecutionEntry()];
  }

  function parseLines(value) {
    return value
      .split(/\r?\n|,/)
      .map((item) => item.trim())
      .filter(Boolean);
  }

  function parseResourceCategories(value) {
    if (Array.isArray(value)) {
      if (value.some((item) => item && typeof item === "object")) {
        return getRequiredResourceCategories(value);
      }

      return value.map((item) => String(item || "").trim()).filter(Boolean);
    }

    return parseLines(value || "");
  }

  function parseCompletionCriteria(value) {
    const criteria = Array.isArray(value)
      ? value
        .map(normalizeCompletionCriterion)
        .map((criterion) => ({
          ...criterion,
          criterion_description: criterion.criterion_description.trim(),
          evidence_note: criterion.evidence_note.trim(),
        }))
        .filter((criterion) => criterion.criterion_description)
      : parseLines(value || "").map((description) =>
          createEmptyCompletionCriterion(description)
        );

    if (criteria.length > MAX_COMPLETION_CRITERIA) {
      throw new Error(
        `Completion criteria are capped at ${MAX_COMPLETION_CRITERIA} items.`
      );
    }

    return criteria;
  }

  function parseExecutionEntries(value) {
    const entries = Array.isArray(value)
      ? value
        .map(normalizeExecutionEntry)
        .map((entry) => ({
          ...entry,
          resource_category: entry.resource_category.trim(),
          target_amount: String(entry.target_amount || "").trim(),
          target_unit: String(entry.target_unit || "").trim(),
          target_needed: buildTargetNeeded(entry),
          current_acquired_amount: "",
          external_coordination_link: "",
          status_proof_note: "",
        }))
        .filter((entry) => entry.target_needed)
      : parseLines(value || "").map((target) =>
          createEmptyExecutionEntry({
            target_needed: target,
            resource_category: "other",
          })
        );

    if (entries.length > MAX_RESOURCE_REQUIREMENTS) {
      throw new Error(
        `Required resources are capped at ${MAX_RESOURCE_REQUIREMENTS} items.`
      );
    }

    return entries;
  }

  function findPersonalProposal(proposalId) {
    const allPersonalItems = [
      ...reviewQueues.issues_to_review,
      ...reviewQueues.solutions_to_review,
      ...reviewQueues.issues_reviewed,
      ...reviewQueues.solutions_reviewed,
    ];

    return allPersonalItems.find((item) => item.id === proposalId);
  }

  const selectedPersonalProposal = selectedProposal?.proposal?.id
    ? findPersonalProposal(selectedProposal.proposal.id)
    : null;

  const selectedBoardCode = selectedProposal?.proposal?.board_code || getBoardForTab(activeTab);
  const selectedIsArchived = Boolean(
    selectedProposal?.proposal?.is_archived ||
      selectedProposal?.proposal?.primary_state === "archived"
  );
  const selectedArchiveRestoreBlocked =
    selectedProposal?.proposal?.archived_reason === "merged" ||
    selectedProposal?.proposal?.archived_reason === "cycle_closed";
  const selectedHighModerationWatch = isHighModerationWatch(
    selectedProposal?.proposal
  );
  const selectedStateLabel =
    selectedProposal?.proposal?.primary_state || (selectedIsArchived ? "archived" : "active");
  const selectedIsAuthor =
    selectedProposal?.proposal?.author_user_id && me?.user_id
      ? selectedProposal.proposal.author_user_id === me.user_id
      : false;
  const selectedAppealAlreadySubmitted = Boolean(
    selectedProposal?.moderator_actions?.some(
      (action) => action.action_type === "appeal_submission"
    )
  );
  const selectedAlreadyReviewed = selectedPersonalProposal?.current_user_reviewed || false;
  const selectedSentimentVote =
    localSentimentVotes[selectedProposal?.proposal?.id] ||
    selectedProposal?.current_user_sentiment_vote ||
    selectedPersonalProposal?.current_user_sentiment_vote ||
    "";
  const selectedMergeVotePresent =
    selectedProposal?.current_user_merge_vote_present ||
    selectedPersonalProposal?.current_user_merge_vote_present ||
    false;
  const selectedMergeVoteTarget =
    selectedProposal?.current_user_merge_target_proposal_id ||
    selectedPersonalProposal?.current_user_merge_target_proposal_id || "";
  const currentMergeVoteTargetId = mergeVoteTargetId || selectedMergeVoteTarget;
  const selectedCanBeReviewed =
    canParticipate &&
    !selectedIsAuthor &&
    !selectedIsArchived &&
    (selectedBoardCode === "issue" || selectedBoardCode === "solution") &&
    !selectedAlreadyReviewed &&
    !unlockStatus?.review_unlocked &&
    (unlockStatus?.required_review_actions || 0) >
      (unlockStatus?.completed_review_actions || 0);

  const currentBoardCode = getBoardForTab(activeTab);
  const unlockProgress = unlockStatus
    ? `${Math.min(
        unlockStatus.completed_review_actions,
        unlockStatus.required_review_actions
      )}/${unlockStatus.required_review_actions}`
    : "0/0";
  const reviewUnlockDenominator =
    (unlockStatus?.required_review_actions || 0) >= STANDARD_REQUIRED_REVIEW_COUNT
      ? STANDARD_REQUIRED_REVIEW_COUNT
      : unlockStatus?.required_review_actions || 0;
  const reviewUnlockProgress = unlockStatus
    ? `${Math.min(
        unlockStatus.completed_review_actions,
        reviewUnlockDenominator
      )}/${reviewUnlockDenominator}`
    : `0/${STANDARD_REQUIRED_REVIEW_COUNT}`;
  const reviewUnlockMessage =
    reviewUnlockDenominator > 0
      ? `Complete ${reviewUnlockProgress} reviews to unlock voting.`
      : "Voting unlocks automatically when no eligible reviews are available.";
  const canSubmitOnCurrentBoard =
    canParticipate &&
    (activeTab === "issues" || activeTab === "solutions") &&
    unlockStatus?.submit_unlocked;
  const solutionTargetAvailable =
    solutionTargetIsPublishedWinner && solutionTargetOptions.length > 0;
  const canSubmitSolutionOnCurrentBoard =
    canSubmitOnCurrentBoard && solutionTargetAvailable;
  const solutionTargetProposal = solutionTargetOptions[0] || null;
  const reviewGateApplies = tabUsesReviewGate(activeTab);
  const needsRequiredReview =
    reviewGateApplies &&
    canParticipate &&
    unlockStatus &&
    !unlockStatus.review_unlocked &&
    unlockStatus.required_review_actions > unlockStatus.completed_review_actions;
  const openingPhaseLocked = Boolean(needsRequiredReview);
  const currentUserSubmission =
    activeTab === "issues" || activeTab === "solutions"
      ? findCurrentUserSubmission(currentBoardCode)
      : null;
  const boardActionsUnlocked =
    canParticipate && !openingPhaseLocked && (activeTab === "issues" || activeTab === "solutions");
  const submissionDockLabel = activeTab === "solutions"
      ? "Submit Solution"
      : "Submit Issue";
  const showSubmissionDockButton =
    boardActionsUnlocked && !currentUserSubmission && Boolean(canSubmitOnCurrentBoard);
  const showSubmissionSectionButton =
    boardActionsUnlocked && Boolean(currentUserSubmission);
  const showingSubmissionView =
    !openingPhaseLocked &&
    submissionPanelOpen &&
    (activeTab === "issues" || activeTab === "solutions");
  const showingSolutionSubmissionView =
    showingSubmissionView && activeTab === "solutions";
  const showSolutionSubmissionIssueDetail =
    showingSolutionSubmissionView &&
    !selectedProposal &&
    !selectedProposalLoading &&
    !selectedExecution &&
    !selectedExecutionLoading &&
    Boolean(solutionTargetProposal);
  const detailDockLabel = showingSolutionSubmissionView ? "Issue" : selectedTitle;
  const showingAccountView = activeTab === "account";
  const canSearchCurrentView = !openingPhaseLocked && items.length > 0;
  const visibleItems = searchQuery.trim()
    ? items.filter((item) => itemMatchesSearch(item, searchQuery))
    : items;
  const shouldSplitReviewedSubmissions =
    activeTab === "issues" || activeTab === "solutions";
  const reviewedBoardItemIds = new Set(
    (activeTab === "solutions"
      ? reviewQueues.solutions_reviewed
      : reviewQueues.issues_reviewed
    ).map((item) => item.id)
  );
  const unreviewedVisibleItems = shouldSplitReviewedSubmissions
    ? visibleItems.filter((item) => !reviewedBoardItemIds.has(getItemId(item)))
    : visibleItems;
  const reviewedVisibleItems = shouldSplitReviewedSubmissions
    ? visibleItems.filter((item) => reviewedBoardItemIds.has(getItemId(item)))
    : [];
  const boardDisplayItems =
    shouldSplitReviewedSubmissions && reviewedVisibleItems.length
      ? [
          ...unreviewedVisibleItems,
          { section_marker: "reviewed_submissions" },
          ...reviewedVisibleItems,
        ]
      : visibleItems;
  const navigationTabs = visibleTabs;
  const primaryNavigationTabs = navigationTabs.filter((tab) =>
    PRIMARY_NAV_TAB_KEYS.has(tab.key)
  );
  const secondaryNavigationTabs = navigationTabs.filter(
    (tab) => !PRIMARY_NAV_TAB_KEYS.has(tab.key)
  );
  const dockColumnCount = 2 + (showSubmissionDockButton ? 1 : 0);
  const navDockActive = navDrawerOpen && frontDrawer === "nav";
  const submissionDockActive = showingSubmissionView;
  const detailDockActive = detailDrawerOpen && frontDrawer === "detail";
  const solutionCompletionCriteria = asArray(solutionForm.completionCriteria);
  const solutionCriteriaAtLimit =
    solutionCompletionCriteria.length >= MAX_COMPLETION_CRITERIA;
  const solutionResourceEntries = asArray(solutionForm.executionTrackingEntries);
  const solutionResourcesAtLimit =
    solutionResourceEntries.length >= MAX_RESOURCE_REQUIREMENTS;
  const canVoteOnSelected =
    canParticipate &&
    selectedProposal?.proposal &&
    (selectedBoardCode === "issue" || selectedBoardCode === "solution") &&
    (selectedIsArchived
      ? unlockStatus?.archive_voting_unlocked
      : unlockStatus?.voting_unlocked);
  const canMergeVoteOnSelected =
    canParticipate &&
    selectedProposal?.proposal &&
    selectedBoardCode === currentBoardCode &&
    (selectedBoardCode === "issue" || selectedBoardCode === "solution") &&
    !selectedIsArchived &&
    !selectedIsAuthor &&
    unlockStatus?.review_unlocked;
  const selectedIsSolutionTargetIssue = Boolean(
    activeTab === "solutions" &&
      selectedProposal?.proposal?.id &&
      solutionTargetOptions.some((issue) => issue.id === selectedProposal.proposal.id)
  );
  const showPersonalReviewControls =
    !selectedIsSolutionTargetIssue &&
    selectedProposal?.proposal &&
    (selectedBoardCode === "issue" || selectedBoardCode === "solution");
  const mergeVoteOptions = (
    selectedBoardCode === "solution" ? solutionOptions : issueOptions
  ).filter(
    (proposal) =>
      proposal.id !== selectedProposal?.proposal?.id && !proposal.is_archived
  );
  const personalVotingPanel = showPersonalReviewControls ? (
    <div className="moderation-box proposal-vote-panel">
      <h4>Your Review</h4>

      {!selectedAlreadyReviewed && selectedCanBeReviewed ? (
        <div className="vote-grid">
          {VOTE_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={
                selectedSentimentVote === option.value ? "active-choice" : ""
              }
              onClick={() =>
                handleSubmitReviewAction(
                  selectedProposal.proposal.id,
                  option.value
                )
              }
              disabled={reviewActionLoading}
            >
              {reviewActionLoading
                ? "Saving..."
                : getVoteOptionLabel(option, selectedBoardCode)}
            </button>
          ))}
        </div>
      ) : selectedIsArchived ? (
        <p className="muted">
          Archived proposals do not count toward required reviews.
        </p>
      ) : selectedIsAuthor ? (
        <p className="muted">
          Your own proposal does not count toward required reviews.
        </p>
      ) : (
        <p className="muted">Review vote credited for this proposal.</p>
      )}

      <h4>Your Sentiment Vote</h4>
      {!canVoteOnSelected ? (
        <p className="muted">
          {selectedIsArchived
            ? "Archive voting unlocks after required reviews are complete."
            : unlockStatus?.voting_open
              ? "Voting unlocks after required reviews are complete."
              : "Voting is closed for this cycle phase."}
        </p>
      ) : (
        <div className="vote-grid">
          {VOTE_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={selectedSentimentVote === option.value ? "active-choice" : ""}
              onClick={() => handleSentimentVote(option.value)}
              disabled={voteLoading}
            >
              {getVoteOptionLabel(option, selectedBoardCode)}
            </button>
          ))}
        </div>
      )}

      {!selectedIsArchived ? (
        <>
          <h4>Possible Duplicate</h4>
          {!canMergeVoteOnSelected ? (
            <p className="muted">
              {selectedIsAuthor
                ? "You cannot link your own proposal."
                : unlockStatus?.review_unlocked
                  ? "Duplicate links are only available on active proposals in the current board."
                  : "Duplicate links unlock after required reviews are complete."}
            </p>
          ) : (
            <>
              <p className="muted small-muted">
                This submission ID: <code>{selectedProposal.proposal.id}</code>
              </p>
              <label className="moderation-field">
                Related Submission ID
                <input
                  value={currentMergeVoteTargetId}
                  onChange={(event) => {
                    setMergeVoteTargetId(event.target.value.trim());
                    if (voteError) setVoteError("");
                  }}
                  list="merge-target-options"
                  maxLength={SUBMISSION_ID_CHARS}
                  placeholder="Paste another active submission ID"
                />
                <datalist id="merge-target-options">
                  {mergeVoteOptions.map((proposal) => (
                    <option key={proposal.id} value={proposal.id}>
                      {proposal.title}
                    </option>
                  ))}
                </datalist>
              </label>

              <button
                type="button"
                className={`duplicate-link-button ${
                  selectedMergeVotePresent ? "active-choice" : ""
                }`}
                onClick={handleMergeVote}
                disabled={voteLoading || !currentMergeVoteTargetId}
              >
                {selectedMergeVotePresent
                  ? "Update Duplicate Link"
                  : "Save Duplicate Link"}
              </button>
            </>
          )}
        </>
      ) : null}

      {voteError ? <div className="error-box">{voteError}</div> : null}
      {voteSuccess ? <div className="success-box">{voteSuccess}</div> : null}
      {reviewActionError ? (
        <div className="error-box">{reviewActionError}</div>
      ) : null}
      {reviewActionSuccess ? (
        <div className="success-box">{reviewActionSuccess}</div>
      ) : null}
    </div>
  ) : null;

  const selectedOutgoingRelationships =
    selectedProposal?.merge_relationships?.outgoing || [];
  const selectedMergeWatch = isMergeWatch(selectedProposal?.proposal);

  const showDistinctionForm =
    selectedProposal?.proposal &&
    canParticipate &&
    selectedIsAuthor &&
    selectedMergeWatch &&
    selectedOutgoingRelationships.length > 0;
  const showDistinctionLocked =
    selectedProposal?.proposal &&
    canParticipate &&
    selectedIsAuthor &&
    !selectedIsArchived &&
    selectedOutgoingRelationships.length > 0 &&
    !selectedMergeWatch;

  const showAuthorAppeal =
    selectedProposal?.proposal &&
    canParticipate &&
    selectedIsArchived &&
    selectedIsAuthor &&
    !selectedArchiveRestoreBlocked &&
    !selectedAppealAlreadySubmitted;
  const showUseArchivedAsDraft =
    selectedProposal?.proposal &&
    canParticipate &&
    selectedIsArchived &&
    (selectedBoardCode === "issue" || selectedBoardCode === "solution");

  const showAppealReviewControls =
    isModerator && activeTab === "appeals" && selectedAppeal?.appeal_id;
  const selectedAppealMustRecuse = Boolean(
    selectedAppeal?.current_moderator_must_recuse
  );
  const selectedAppealRestoreBlocked =
    selectedAppeal?.archived_reason === "merged" ||
    selectedAppeal?.archived_reason === "cycle_closed";

  const showStartReconsideration =
    isModerator &&
    activeTab === "archive" &&
    selectedProposal?.proposal &&
    selectedIsArchived &&
    !selectedArchiveRestoreBlocked;

  const showReconsiderationReviewControls =
    isModerator &&
    activeTab === "reconsiderations" &&
    selectedReconsideration?.reconsideration_id;
  const selectedReconsiderationRestoreBlocked =
    selectedReconsideration?.previous_archived_reason === "merged" ||
    selectedReconsideration?.previous_archived_reason === "cycle_closed";

  const showModerationControls =
    isModerator &&
    activeTab === "reviewQueue" &&
    selectedProposal?.proposal?.primary_state === "active" &&
    (selectedHighModerationWatch ||
      selectedProposal?.proposal?.review_reason === "frozen_review");
  const showModerationObservationOnly =
    isModerator &&
    activeTab === "reviewQueue" &&
    selectedProposal?.proposal?.primary_state === "active" &&
    !showModerationControls;
  const selectedIsFrozen = selectedProposal?.proposal?.review_reason === "frozen_review";

  const mergeOptions = [
    ...(selectedProposal?.merge_relationships?.outgoing || []).map((rel) => ({
      id: rel.target_proposal_id,
      label: `Outgoing link: ${rel.target_title}`,
      proposal: findCurrentListProposal(rel.target_proposal_id),
      relationship: rel,
    })),
    ...(selectedProposal?.merge_relationships?.incoming || []).map((rel) => ({
      id: rel.source_proposal_id,
      label: `Incoming link: ${rel.source_title}`,
      proposal: findCurrentListProposal(rel.source_proposal_id),
      relationship: rel,
    })),
  ];

  const selectedMergeOption = mergeOptions.find((option) => option.id === mergeTargetId);
  const selectedMergeProposal = selectedMergeOption?.proposal || null;
  const selectedMergeRelationship = selectedMergeOption?.relationship || null;
  const selectedMergeCurrentTotal = selectedProposal?.proposal
    ? getTotalVoteCount(selectedProposal.proposal)
    : null;
  const selectedMergeTargetTotal = selectedMergeProposal
    ? getTotalVoteCount(selectedMergeProposal)
    : null;
  const mergeDirectionKnown =
    selectedMergeCurrentTotal !== null && selectedMergeTargetTotal !== null;
  const mergeTotalsEqual =
    mergeDirectionKnown && selectedMergeCurrentTotal === selectedMergeTargetTotal;
  const mergeArchiveProposal =
    mergeDirectionKnown && !mergeTotalsEqual
      ? selectedMergeCurrentTotal < selectedMergeTargetTotal
        ? selectedProposal.proposal
        : selectedMergeProposal
      : null;
  const mergeSurvivingProposal =
    mergeDirectionKnown && !mergeTotalsEqual
      ? selectedMergeCurrentTotal > selectedMergeTargetTotal
        ? selectedProposal.proposal
        : selectedMergeProposal
      : null;
  const mergeArchiveTotal =
    mergeArchiveProposal?.id === selectedProposal?.proposal?.id
      ? selectedMergeCurrentTotal
      : selectedMergeTargetTotal;
  const mergeSurvivingTotal =
    mergeSurvivingProposal?.id === selectedProposal?.proposal?.id
      ? selectedMergeCurrentTotal
      : selectedMergeTargetTotal;
  const mergePairThresholdKnown = Boolean(
    selectedMergeRelationship &&
      typeof selectedMergeRelationship.source_to_target_high_merge_watch === "boolean" &&
      typeof selectedMergeRelationship.target_to_source_high_merge_watch === "boolean"
  );
  const mergePairThresholdMet = Boolean(
    selectedMergeRelationship?.source_to_target_high_merge_watch ||
      selectedMergeRelationship?.target_to_source_high_merge_watch
  );
  const mergeBlockedByThreshold =
    mergeDirectionKnown &&
    !mergeTotalsEqual &&
    mergePairThresholdKnown &&
    !mergePairThresholdMet;
  const mergeThresholdDirections = selectedMergeRelationship
    ? [
        selectedMergeRelationship.source_to_target_high_merge_watch
          ? `${selectedMergeRelationship.source_title} -> ${selectedMergeRelationship.target_title}`
          : null,
        selectedMergeRelationship.target_to_source_high_merge_watch
          ? `${selectedMergeRelationship.target_title} -> ${selectedMergeRelationship.source_title}`
          : null,
      ]
        .filter(Boolean)
        .join(", ")
    : "";
  const showMergeControls =
    isModerator &&
    activeTab === "reviewQueue" &&
    selectedProposal?.proposal?.primary_state === "active" &&
    mergeOptions.length > 0;

  const showExecutionRecordControls =
    isModerator &&
    selectedBoardCode === "solution" &&
    !selectedIsArchived &&
    selectedProposal?.proposal?.id;

  if (!sessionChecked) {
    return (
      <div className="app-shell">
        <div className="auth-card">
          <h1>Keystone</h1>
          <p className="muted">Checking session...</p>
        </div>
      </div>
    );
  }

  if (!me) {
    return (
      <div className="app-shell">
        <div className="auth-card">
          <h1>Keystone</h1>

          <div className="auth-toggle">
            <button
              className={authMode === "login" ? "active" : ""}
              onClick={() => {
                setAuthMode("login");
                setAuthError("");
                setAuthSuccess("");
              }}
              type="button"
            >
              Login
            </button>
            <button
              className={authMode === "register" ? "active" : ""}
              onClick={() => {
                setAuthMode("register");
                setAuthError("");
                setAuthSuccess("");
              }}
              type="button"
            >
              Register
            </button>
          </div>

          <form onSubmit={handleAuthSubmit} className="auth-form">
            {authMode !== "resetConfirm" ? (
              <label>
                Email
                <input
                  type="email"
                  value={email}
                  onChange={(event) => setEmail(event.target.value)}
                  maxLength={MAX_EMAIL_CHARS}
                  required
                />
              </label>
            ) : null}

            {authMode === "login" || authMode === "register" ? (
              <label>
                Password
                <input
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  maxLength={MAX_PASSWORD_CHARS}
                  required
                />
              </label>
            ) : null}

            {authMode === "resetConfirm" ? (
              <>
                <label>
                  Reset Token
                  <input
                    value={passwordResetToken}
                    onChange={(event) => setPasswordResetToken(event.target.value)}
                    maxLength={MAX_TOKEN_CHARS}
                    required
                  />
                </label>

                <label>
                  New Password
                  <input
                    type="password"
                    value={passwordResetNewPassword}
                    onChange={(event) =>
                      setPasswordResetNewPassword(event.target.value)
                    }
                    maxLength={MAX_PASSWORD_CHARS}
                    required
                  />
                </label>
              </>
            ) : null}

            {authError ? <div className="error-box">{authError}</div> : null}
            {authSuccess ? <div className="success-box">{authSuccess}</div> : null}

            <button type="submit" disabled={authLoading}>
              {authLoading
                ? "Working..."
                : authMode === "login"
                  ? "Login"
                  : authMode === "register"
                    ? "Register"
                    : authMode === "resetRequest"
                      ? "Send Reset Instructions"
                      : "Reset Password"}
            </button>

            <div className="auth-secondary-actions">
              {authMode === "login" ? (
                <button
                  type="button"
                  onClick={() => {
                    setAuthMode("resetRequest");
                    setAuthError("");
                    setAuthSuccess("");
                  }}
                >
                  Forgot password?
                </button>
              ) : null}
              {authMode === "resetRequest" || authMode === "resetConfirm" ? (
                <button
                  type="button"
                  onClick={() => {
                    setAuthMode("login");
                    setAuthError("");
                    setAuthSuccess("");
                  }}
                >
                  Back to login
                </button>
              ) : null}
            </div>
          </form>

          {SHOW_PROTOTYPE_ACCOUNTS ? (
            <div className="prototype-accounts">
              <p className="muted">Prototype accounts</p>
              <div className="prototype-account-list">
                {PROTOTYPE_ACCOUNTS.map((account) => (
                  <button
                    type="button"
                    className="prototype-account"
                    key={account.email}
                    onClick={() => handleSelectPrototypeAccount(account)}
                  >
                    <strong>{account.roleLabel}</strong>
                    <span>{account.email}</span>
                  </button>
                ))}
              </div>
              <p className="muted small-muted">Password: {PROTOTYPE_PASSWORD}</p>
            </div>
          ) : null}
        </div>
      </div>
    );
  }

  return (
    <div className="app-shell app-shell-board">
      <header className="topbar">
        <div>
          <h1>Keystone</h1>
        </div>
      </header>

      {!me.email_verified ? (
        <section className="panel verification-panel">
          <div className="tool-section-header">
            <h2>Email Verification</h2>
            <span className="state-pill subtle-pill">Required</span>
          </div>

          <form className="proposal-form" onSubmit={handleVerifyEmail}>
            <label>
              Verification Token
              <input
                value={verificationToken}
                onChange={(event) => setVerificationToken(event.target.value)}
                maxLength={MAX_TOKEN_CHARS}
                required
              />
            </label>

            {verificationError ? (
              <div className="error-box">{verificationError}</div>
            ) : null}
            {verificationSuccess ? (
              <div className="success-box">{verificationSuccess}</div>
            ) : null}

            <div className="action-row">
              <button type="submit" disabled={verificationLoading}>
                {verificationLoading ? "Verifying..." : "Verify Email"}
              </button>
              <button
                type="button"
                onClick={handleRequestVerificationToken}
                disabled={verificationLoading}
              >
                Send Verification
              </button>
            </div>
          </form>
        </section>
      ) : null}

      <main className="board-layout">
        <section className="panel board-panel">
          <div className="panel-header">
            {needsRequiredReview ? (
              <p className="muted review-unlock-message">{reviewUnlockMessage}</p>
            ) : null}
            <h2
              className={
                !showingSubmissionView &&
                (activeTab === "issues" || activeTab === "solutions")
                  ? "review-feed-title"
                  : undefined
              }
            >
              {showingSubmissionView
                ? activeTab === "issues"
                  ? "Submit Issue"
                  : "Submit Solution"
                : pageTitle}
            </h2>
            {(activeTab === "issues" ||
              activeTab === "solutions" ||
              activeTab === "reviewPool") &&
            needsRequiredReview ? (
              <div className="proposal-badge-stack">
                <span className="state-pill subtle-pill">{unlockProgress}</span>
              </div>
            ) : null}
            {showingSolutionSubmissionView && solutionTargetProposal ? (
              <div className="solution-target-title">
                <span>Issue</span>
                <strong>{solutionTargetProposal.title}</strong>
              </div>
            ) : null}
            {!openingPhaseLocked &&
            !showingAccountView &&
            canSearchCurrentView &&
            !showingSubmissionView ? (
              <div className="panel-actions">
                <button
                  type="button"
                  onClick={() => {
                    setSubmissionPanelOpen(false);
                    setSearchPanelOpen((current) => !current);
                  }}
                >
                  {searchPanelOpen ? "Close Search" : "Search"}
                </button>
              </div>
            ) : null}
          </div>

          {unlockError ? <div className="error-box">{unlockError}</div> : null}

          {showingAccountView ? (
            <section className="tool-section account-panel">
              <div className="user-chip account-chip">
                <strong>{me.email}</strong>
                <span>{formatRoleLabel(me.role_code)}</span>
              </div>

              <button type="button" onClick={handleLogout}>
                Logout
              </button>
            </section>
          ) : null}

          {!openingPhaseLocked && !showingSubmissionView && searchPanelOpen ? (
            <div className="tool-section">
              <label className="moderation-field">
                Search This View
                <input
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  maxLength={MAX_SEARCH_CHARS}
                  placeholder="Search titles and descriptions"
                />
              </label>
            </div>
          ) : null}

          {!showingSubmissionView && activeTab === "solutions" && solutionTargetProposal ? (
            <div className="solution-target-card">
              <span className="state-pill solution-target-pill">Problem Being Solved</span>
              <strong>{solutionTargetProposal.title}</strong>
            </div>
          ) : null}

          {showingSubmissionView ? (
            <section className="tool-section submission-panel">
              {!canSubmitOnCurrentBoard ? (
                <p className="muted">
                  {unlockStatus?.submission_open
                    ? "Submission opens after the review requirement is complete."
                    : "Submission is closed for this cycle phase."}
                </p>
              ) : null}

              {activeTab === "issues" ? (
                <form className="proposal-form" onSubmit={handleSubmitIssue}>
                  <label>
                    Title
                    <input
                      value={issueForm.title}
                      onChange={(event) => updateIssueForm("title", event.target.value)}
                      maxLength={MAX_TITLE_CHARS}
                      required
                    />
                  </label>
                  <label>
                    Problem Description
                    <textarea
                      value={issueForm.problemDescription}
                      onChange={(event) =>
                        updateIssueForm("problemDescription", event.target.value)
                      }
                      maxLength={MAX_LONG_TEXT_CHARS}
                      rows={4}
                      required
                    />
                  </label>
                  <label>
                    Affected People or Scope
                    <input
                      value={issueForm.affectedScope}
                      onChange={(event) =>
                        updateIssueForm("affectedScope", event.target.value)
                      }
                      maxLength={MAX_SCOPE_CHARS}
                      required
                    />
                  </label>
                  <label>
                    Why It Matters
                    <textarea
                      value={issueForm.whyItMatters}
                      onChange={(event) =>
                        updateIssueForm("whyItMatters", event.target.value)
                      }
                      maxLength={MAX_LONG_TEXT_CHARS}
                      rows={3}
                      required
                    />
                  </label>

                  <button type="submit" disabled={!canSubmitOnCurrentBoard || submitLoading}>
                    {submitLoading ? "Submitting..." : "Submit Issue"}
                  </button>
                </form>
              ) : (
                <form
                  className="proposal-form solution-blueprint-form"
                  onSubmit={handleSubmitSolution}
                >
                  <label>
                    Title
                    <input
                      value={solutionForm.title}
                      onChange={(event) => updateSolutionForm("title", event.target.value)}
                      maxLength={MAX_TITLE_CHARS}
                      required
                    />
                  </label>
                  {!solutionTargetAvailable ? (
                    <p className="muted">
                      Solution submissions open after the first winning issue is published.
                    </p>
                  ) : null}
                  <label>
                    Action Description
                    <textarea
                      value={solutionForm.actionDescription}
                      onChange={(event) =>
                        updateSolutionForm("actionDescription", event.target.value)
                      }
                      maxLength={MAX_LONG_TEXT_CHARS}
                      rows={4}
                      required
                    />
                  </label>
                  <label>
                    Why This Solves It
                    <textarea
                      value={solutionForm.whyThisSolvesIt}
                      onChange={(event) =>
                        updateSolutionForm("whyThisSolvesIt", event.target.value)
                      }
                      maxLength={MAX_SOLUTION_FIT_CHARS}
                      rows={3}
                      required
                    />
                  </label>
                  <fieldset className="structured-fieldset">
                    <div className="structured-fieldset-heading">
                      <span>
                        Required Resources ({solutionResourceEntries.length}/
                        {MAX_RESOURCE_REQUIREMENTS})
                      </span>
                      <button
                        type="button"
                        onClick={addSolutionExecutionEntry}
                        disabled={solutionResourcesAtLimit}
                      >
                        {solutionResourcesAtLimit ? "64 Max" : "Add Resource"}
                      </button>
                    </div>
                    <div className="structured-list">
                      {solutionResourceEntries.map(
                        (entry, index) => (
                          <div className="structured-item" key={`solution-entry-${index}`}>
                            <div className="structured-item-header">
                              <span className="structured-index">{index + 1}</span>
                              <strong>Required Resource</strong>
                            </div>
                            <div className="structured-row">
                              <label>
                                Resource Type
                                <select
                                  value={entry.resource_category || "other"}
                                  onChange={(event) =>
                                    updateSolutionExecutionEntry(
                                      index,
                                      "resource_category",
                                      event.target.value
                                    )
                                  }
                                  required
                                >
                                  {RESOURCE_CATEGORY_OPTIONS.map((category) => (
                                    <option key={category.value} value={category.value}>
                                      {category.label}
                                    </option>
                                  ))}
                                </select>
                              </label>
                              <label>
                                Amount Required
                                <input
                                  value={entry.target_amount || ""}
                                  onChange={(event) =>
                                    updateSolutionExecutionEntry(
                                      index,
                                      "target_amount",
                                      event.target.value
                                    )
                                  }
                                  inputMode="decimal"
                                  maxLength={MAX_RESOURCE_AMOUNT_CHARS}
                                  placeholder="Example: 12000"
                                  required
                                />
                              </label>
                            </div>
                            <div className="structured-row structured-row-single">
                              <label>
                                Unit
                                <input
                                  value={entry.target_unit || ""}
                                  onChange={(event) =>
                                    updateSolutionExecutionEntry(
                                      index,
                                      "target_unit",
                                      event.target.value
                                    )
                                  }
                                  maxLength={MAX_RESOURCE_UNIT_CHARS}
                                  placeholder="Example: dollars, volunteers, hours"
                                  required
                                />
                              </label>
                            </div>
                            <button
                              type="button"
                              className="quiet-button"
                              onClick={() => removeSolutionExecutionEntry(index)}
                              disabled={solutionResourceEntries.length <= 1}
                            >
                              Remove
                            </button>
                          </div>
                        )
                      )}
                    </div>
                    <div className="resource-summary-box">
                      <strong>Required Types</strong>
                      <div className="proposal-badge-stack">
                        {getRequiredResourceCategories(
                          solutionResourceEntries
                        ).map((category) => (
                          <span className="state-pill subtle-pill" key={category}>
                            {formatResourceCategory(category)}
                          </span>
                        ))}
                      </div>
                    </div>
                  </fieldset>

                  <fieldset className="structured-fieldset compact-criteria-fieldset">
                    <div className="structured-fieldset-heading">
                      <span>
                        Completion Criteria ({solutionCompletionCriteria.length}/
                        {MAX_COMPLETION_CRITERIA})
                      </span>
                      <button
                        type="button"
                        onClick={addSolutionCriterion}
                        disabled={solutionCriteriaAtLimit}
                      >
                        {solutionCriteriaAtLimit ? "8 Max" : "Add Criterion"}
                      </button>
                    </div>
                    <div className="compact-criteria-list">
                      {solutionCompletionCriteria.map((criterion, index) => (
                        <div
                          className="compact-criteria-item"
                          key={`solution-criterion-${index}`}
                        >
                          <span className="structured-index">{index + 1}</span>
                          <input
                            aria-label={`Completion criterion ${index + 1}`}
                            value={criterion.criterion_description || ""}
                            onChange={(event) =>
                              updateSolutionCriterion(
                                index,
                                "criterion_description",
                                event.target.value
                              )
                            }
                            maxLength={MAX_COMPLETION_CRITERION_CHARS}
                            placeholder="Done when..."
                            required
                          />
                          <button
                            type="button"
                            className="quiet-button"
                            onClick={() => removeSolutionCriterion(index)}
                            disabled={solutionCompletionCriteria.length <= 1}
                          >
                            Remove
                          </button>
                        </div>
                      ))}
                    </div>
                  </fieldset>

                  <button
                    type="submit"
                    disabled={!canSubmitSolutionOnCurrentBoard || submitLoading}
                  >
                    {submitLoading ? "Submitting..." : "Submit Solution"}
                  </button>
                </form>
              )}

              {submitError ? <div className="error-box">{submitError}</div> : null}
              {submitSuccess ? <div className="success-box">{submitSuccess}</div> : null}
            </section>
          ) : null}

          {!showingSubmissionView && activeTab === "outcomes" && isModerator ? (
            <section className="tool-section">
              <div className="tool-section-header">
                <h3>Cycle Closeout</h3>
                <span className="state-pill subtle-pill">
                  {outcomeData?.can_resolve ? "Ready" : "Waiting"}
                </span>
              </div>

              <div className="compact-card">
                <strong>Cycle {outcomeData?.cycle?.cycle_number || "current"}</strong>
                <p>
                  Voting ends {outcomeData?.cycle?.voting_ends_at || "after the voting window"}.
                </p>
                {outcomeData?.results?.length ? (
                  <p className="muted">Published results are already stored.</p>
                ) : null}
              </div>

              {outcomeResolveError ? (
                <div className="error-box">{outcomeResolveError}</div>
              ) : null}
              {outcomeResolveSuccess ? (
                <div className="success-box">{outcomeResolveSuccess}</div>
              ) : null}

              <button
                type="button"
                onClick={handleResolveCurrentCycle}
                disabled={!outcomeData?.can_resolve || outcomeResolveLoading}
              >
                {outcomeResolveLoading ? "Resolving..." : "Resolve & Publish"}
              </button>
            </section>
          ) : null}

          {!showingSubmissionView && activeTab === "trustReview" && isModerator ? (
            <section className="tool-section">
              <div className="tool-section-header">
                <h3>Trust Review</h3>
                <span className="state-pill subtle-pill">{items.length} open</span>
              </div>
              {trustResolveError ? (
                <div className="error-box">{trustResolveError}</div>
              ) : null}
              {trustResolveSuccess ? (
                <div className="success-box">{trustResolveSuccess}</div>
              ) : null}
            </section>
          ) : null}

          {!showingSubmissionView && !showingAccountView && itemsLoading ? <p>Loading...</p> : null}
          {!showingSubmissionView && !showingAccountView && itemsError ? <div className="error-box">{itemsError}</div> : null}

          {!showingSubmissionView && !showingAccountView && !itemsLoading && !itemsError && items.length === 0 ? (
            <p className="muted">
              {activeTab === "reviewPool"
                ? "No required reviews are waiting here."
                : "Nothing to show here yet."}
            </p>
          ) : null}
          {!showingSubmissionView && !showingAccountView && !itemsLoading && !itemsError && items.length > 0 && visibleItems.length === 0 ? (
            <p className="muted">No matches found.</p>
          ) : null}

          {!showingSubmissionView && !showingAccountView ? (
          <div className="proposal-list">
            {boardDisplayItems.map((item) => {
              if (item?.section_marker === "reviewed_submissions") {
                return (
                  <div
                    className="board-section-heading"
                    key="reviewed-submissions-heading"
                  >
                    <h3>Reviewed Submissions</h3>
                  </div>
                );
              }

              const id = getItemId(item);
              const isReviewedBoardItem =
                shouldSplitReviewedSubmissions && reviewedBoardItemIds.has(id);
              const proposalCardClassName = `proposal-card${
                shouldSplitReviewedSubmissions && !isReviewedBoardItem
                  ? " proposal-card-unreviewed"
                  : ""
              }`;
              const state = getItemState(item);
              const publicStateLabel = getPublicStateLabel(item);
              const description = getItemDescription(item);
              const counts = getItemCounts(item);
              const extraBadge = getExtraBadge(item);
              const isCycleResult = Boolean(item?.result_status);
              const showCounts =
                (isModerator || isCycleResult) &&
                !item?.appeal_id &&
                !item?.reconsideration_id &&
                !item?.solution_proposal_id &&
                (!isCycleResult || item?.winning_proposal);

              if (activeTab === "reviewPool") {
                const reviewVote = localSentimentVotes[id] || "";
                return (
                  <div
                    key={item?.appeal_id || item?.reconsideration_id || id}
                    className="proposal-card required-review-card"
                  >
                    <div className="proposal-card-top">
                      <h3>{getItemTitle(item)}</h3>
                      <span className="state-pill subtle-pill">
                        {formatActionType(currentBoardCode)}
                      </span>
                    </div>

                    {description ? <p className="proposal-snippet">{description}</p> : null}

                    <div className="vote-grid">
                      {VOTE_OPTIONS.map((option) => (
                        <button
                          key={option.value}
                          type="button"
                          className={reviewVote === option.value ? "active-choice" : ""}
                          onClick={() => handleSubmitReviewAction(id, option.value)}
                          disabled={reviewActionLoading}
                        >
                          {reviewActionLoading
                            ? "Saving..."
                            : getVoteOptionLabel(option, currentBoardCode)}
                        </button>
                      ))}
                    </div>
                  </div>
                );
              }

              if (activeTab === "trustReview") {
                const detailRows = [
                  item.user_email ? `User: ${item.user_email}` : null,
                  item.proposal_title ? `Submission: ${item.proposal_title}` : null,
                  item.related_proposal_title
                    ? `Related: ${item.related_proposal_title}`
                    : null,
                  item.client_ip_hint ? `Network: ${item.client_ip_hint}` : null,
                ].filter(Boolean);

                return (
                  <div
                    key={id}
                    className="proposal-card required-review-card"
                  >
                    <div className="proposal-card-top">
                      <h3>{getItemTitle(item)}</h3>
                      <div className="proposal-badge-stack">
                        <span className="state-pill subtle-pill">
                          {formatActionType(item.severity)}
                        </span>
                        <span className="state-pill subtle-pill">
                          {formatActionType(item.status)}
                        </span>
                      </div>
                    </div>

                    {description ? <p className="proposal-snippet">{description}</p> : null}
                    {detailRows.length ? (
                      <div className="proposal-counts">
                        {detailRows.map((row) => (
                          <span key={row}>{row}</span>
                        ))}
                      </div>
                    ) : null}
                    {item.details ? (
                      <details className="more-info-panel">
                        <summary>More info</summary>
                        <pre>{JSON.stringify(item.details, null, 2)}</pre>
                      </details>
                    ) : null}
                    <div className="action-row">
                      <button
                        type="button"
                        onClick={() => handleResolveTrustFlag(id, "acknowledged")}
                        disabled={trustResolveLoading}
                      >
                        {trustResolveLoading ? "Saving..." : "Acknowledge"}
                      </button>
                      <button
                        type="button"
                        className="quiet-button"
                        onClick={() => handleResolveTrustFlag(id, "dismissed")}
                        disabled={trustResolveLoading}
                      >
                        Dismiss
                      </button>
                    </div>
                  </div>
                );
              }

              return (
                <button
                  key={item?.appeal_id || item?.reconsideration_id || id}
                  className={proposalCardClassName}
                  type="button"
                  disabled={isCycleResult && !item?.winning_proposal_id}
                  onClick={() =>
                    activeTab === "implementations"
                      ? handleSelectExecution(id)
                      : handleSelectProposal(id)
                  }
                >
                  <div className="proposal-card-top">
                    <h3>{getItemTitle(item)}</h3>
                    <div className="proposal-badge-stack">
                      {extraBadge ? (
                        <span className="state-pill subtle-pill">{extraBadge}</span>
                      ) : null}
                      {isModerator && state ? (
                        <span className={`state-pill state-${state}`}>{state}</span>
                      ) : null}
                      {!isModerator && publicStateLabel ? (
                        <span className="state-pill subtle-pill">{publicStateLabel}</span>
                      ) : null}
                    </div>
                  </div>

                  {description ? <p className="proposal-snippet">{description}</p> : null}

                  {showCounts ? (
                    <div className="proposal-counts">
                      <span>Support: {counts.support}</span>
                      <span>Not fit: {counts.notFit}</span>
                      <span>Unclear: {counts.unclear}</span>
                      <span>Unsafe: {counts.unsafe}</span>
                      <span>Merge: {counts.merge}</span>
                      {item?.classification ? (
                        <span>
                          Ratio:{" "}
                          {item.support_ratio == null
                            ? "n/a"
                            : item.support_ratio.toFixed(3)}
                        </span>
                      ) : null}
                    </div>
                  ) : null}
                </button>
              );
            })}
          </div>
          ) : null}
        </section>
      </main>

      <section className="bottom-nav-drawer-wrapper">
        <div
          className={`bottom-nav-drawer ${
            navDrawerOpen && navigationTabs.length > 0 ? "open" : ""
          } ${
            frontDrawer === "nav" ? "front" : "back"
          }`}
        >
          <div className="bottom-nav-drawer-content">
            <div className="nav-primary-row">
              {primaryNavigationTabs.map((tab) => (
                <button
                  key={tab.key}
                  type="button"
                  className={`tab-button tab-button-primary ${
                    activeTab === tab.key ? "active" : ""
                  }`}
                  onClick={() => {
                    handleTabChange(tab.key);
                    setNavDrawerOpen(false);
                    setFrontDrawer("nav");
                  }}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            <div className="nav-secondary-row">
              {secondaryNavigationTabs.map((tab) => (
                <button
                  key={tab.key}
                  type="button"
                  className={`tab-button tab-button-secondary ${
                    activeTab === tab.key ? "active" : ""
                  } ${tab.moderatorOnly ? "tab-button-moderator" : ""}`}
                  onClick={() => {
                    handleTabChange(tab.key);
                    setNavDrawerOpen(false);
                    setFrontDrawer("nav");
                  }}
                >
                  {tab.label}
                </button>
              ))}
              {showSubmissionSectionButton ? (
                <button
                  type="button"
                  className="tab-button tab-button-secondary submission-tab-button"
                  onClick={handleSubmissionButton}
                >
                  Submission
                </button>
              ) : null}
            </div>
          </div>
        </div>

        <div
          className={`bottom-detail-drawer ${detailDrawerOpen ? "open" : ""} ${
            frontDrawer === "detail" ? "front" : "back"
          }`}
        >
          <div className="bottom-detail-drawer-content">
            {!selectedProposal &&
            !selectedProposalLoading &&
            !selectedExecution &&
            !selectedExecutionLoading &&
            !showSolutionSubmissionIssueDetail ? (
              <p className="muted">
                {showingSolutionSubmissionView
                  ? "No winning issue is available for this solution cycle."
                  : "Select an item to inspect it."}
              </p>
            ) : null}

            {selectedProposalLoading ? <p>Loading proposal detail...</p> : null}
            {selectedExecutionLoading ? <p>Loading implementation record...</p> : null}

            {selectedProposalError ? (
              <div className="error-box">{selectedProposalError}</div>
            ) : null}
            {selectedExecutionError ? (
              <div className="error-box">{selectedExecutionError}</div>
            ) : null}

            {showSolutionSubmissionIssueDetail ? (
              <div className="detail-card detail-card-drawer solution-submission-issue-detail">
                <span className="state-pill solution-target-pill">
                  Problem Being Solved
                </span>
                <h3>{solutionTargetProposal.title}</h3>

                <div className="detail-grid">
                  <div>
                    <strong>Status:</strong>{" "}
                    {formatActionType(
                      solutionTargetProposal.primary_state ||
                        solutionTargetProposal.status ||
                        "archived"
                    )}
                  </div>
                </div>

                {solutionTargetProposal.problem_description ? (
                  <>
                    <h4>Problem</h4>
                    <p>{solutionTargetProposal.problem_description}</p>
                  </>
                ) : null}

                {solutionTargetProposal.affected_scope ? (
                  <>
                    <h4>Affected People or Scope</h4>
                    <p>{solutionTargetProposal.affected_scope}</p>
                  </>
                ) : null}

                {solutionTargetProposal.why_it_matters ? (
                  <>
                    <h4>Why It Matters</h4>
                    <p>{solutionTargetProposal.why_it_matters}</p>
                  </>
                ) : null}
              </div>
            ) : null}

            {selectedExecution ? (
              <div className="detail-card detail-card-drawer">
                <h3>{selectedExecution.title}</h3>

                <div className="detail-grid">
                  <div>
                    <strong>Status:</strong> {formatActionType(selectedExecution.status)}
                  </div>
                  <div>
                    <strong>Issue:</strong> {selectedExecution.parent_issue_title}
                  </div>
                  <div>
                    <strong>Created:</strong> {selectedExecution.created_at}
                  </div>
                  <div>
                    <strong>Updated:</strong> {selectedExecution.updated_at}
                  </div>
                </div>

                <h4>Action Description</h4>
                <p>{selectedExecution.action_description}</p>

                {asArray(selectedExecution.required_resource_categories).length ? (
                  <>
                    <h4>Required Resources</h4>
                    <div className="proposal-badge-stack">
                      {asArray(selectedExecution.required_resource_categories).map(
                        (category) => (
                          <span className="state-pill subtle-pill" key={category}>
                            {formatResourceCategory(category)}
                          </span>
                        )
                      )}
                    </div>
                  </>
                ) : null}

                {asArray(selectedExecution.completion_criteria).length ? (
                  <>
                    <h4>Completion Criteria</h4>
                    <div className="relationship-block">
                      {asArray(selectedExecution.completion_criteria).map(
                        (criterion, index) => (
                          <div
                            className="relationship-card"
                            key={`execution-criterion-${index}`}
                          >
                            <strong>{getCriterionDescription(criterion)}</strong>
                            {getCriterionStatus(criterion) ? (
                              <p className="muted">
                                {formatActionType(getCriterionStatus(criterion))}
                              </p>
                            ) : null}
                            {getCriterionEvidence(criterion) ? (
                              <p>{getCriterionEvidence(criterion)}</p>
                            ) : null}
                          </div>
                        )
                      )}
                    </div>
                  </>
                ) : null}

                {asArray(selectedExecution.execution_tracking_entries).length ? (
                  <>
                    <h4>Resource Tracking</h4>
                    <div className="relationship-block">
                      {asArray(selectedExecution.execution_tracking_entries).map(
                        (entry, index) => (
                          <div
                            className="relationship-card"
                            key={`execution-entry-${index}`}
                          >
                            <strong>
                              {formatResourceCategory(entry?.resource_category || "other")}
                            </strong>
                            <p>
                              <strong>Needed:</strong>{" "}
                              {entry?.target_needed || "Not specified"}
                            </p>
                            {entry?.current_acquired_amount ? (
                              <p>
                                <strong>Acquired:</strong>{" "}
                                {entry.current_acquired_amount}
                              </p>
                            ) : null}
                            {entry?.external_coordination_link ? (
                              <p>
                                <a
                                  href={entry.external_coordination_link}
                                  target="_blank"
                                  rel="noreferrer"
                                >
                                  Coordination Link
                                </a>
                              </p>
                            ) : null}
                            {getExecutionNote(entry) ? (
                              <p>{getExecutionNote(entry)}</p>
                            ) : null}
                          </div>
                        )
                      )}
                    </div>
                  </>
                ) : null}

                {isModerator ? (
                  <form className="moderation-box" onSubmit={handleSaveExecutionUpdate}>
                    <h4>Update Implementation</h4>

                    <label className="moderation-field">
                      Status
                      <select
                        value={executionEditStatus}
                        onChange={(event) => setExecutionEditStatus(event.target.value)}
                      >
                        <option value="active">Active</option>
                        <option value="paused">Paused</option>
                        <option value="completed">Completed</option>
                        <option value="cancelled">Cancelled</option>
                      </select>
                    </label>

                    {executionCriteriaDraft.length ? (
                      <>
                        <h4>Completion Updates</h4>
                        <div className="relationship-block">
                          {executionCriteriaDraft.map((criterion, index) => (
                            <div
                              className="relationship-card"
                              key={`criteria-update-${index}`}
                            >
                              <strong>{getCriterionDescription(criterion)}</strong>
                              <label className="moderation-field">
                                Status
                                <select
                                  value={getCriterionStatus(criterion) || "not_started"}
                                  onChange={(event) =>
                                    updateExecutionCriterion(
                                      index,
                                      "completion_status",
                                      event.target.value
                                    )
                                  }
                                >
                                  <option value="not_started">Not Started</option>
                                  <option value="in_progress">In Progress</option>
                                  <option value="blocked">Blocked</option>
                                  <option value="completed">Completed</option>
                                </select>
                              </label>
                              <label className="moderation-field">
                                Evidence
                                <textarea
                                  value={getCriterionEvidence(criterion) || ""}
                                  onChange={(event) =>
                                    updateExecutionCriterion(
                                      index,
                                      "evidence_note",
                                      event.target.value
                                    )
                                  }
                                  maxLength={MAX_NOTE_CHARS}
                                  rows={3}
                                />
                              </label>
                            </div>
                          ))}
                        </div>
                      </>
                    ) : null}

                    {executionEntriesDraft.length ? (
                      <>
                        <h4>Resource Updates</h4>
                        <div className="relationship-block">
                          {executionEntriesDraft.map((entry, index) => (
                            <div
                              className="relationship-card"
                              key={`entry-update-${index}`}
                            >
                              <strong>
                                {formatResourceCategory(entry?.resource_category || "other")}
                              </strong>
                              <p>
                                <strong>Needed:</strong>{" "}
                                {entry?.target_needed || "Not specified"}
                              </p>
                              <label className="moderation-field">
                                Acquired
                                <input
                                  value={entry?.current_acquired_amount || ""}
                                  onChange={(event) =>
                                    updateExecutionEntry(
                                      index,
                                      "current_acquired_amount",
                                      event.target.value
                                    )
                                  }
                                  maxLength={MAX_RESOURCE_AMOUNT_CHARS}
                                />
                              </label>
                              <label className="moderation-field">
                                Coordination Link
                                <input
                                  value={entry?.external_coordination_link || ""}
                                  onChange={(event) =>
                                    updateExecutionEntry(
                                      index,
                                      "external_coordination_link",
                                      event.target.value
                                    )
                                  }
                                  maxLength={MAX_LINK_CHARS}
                                />
                              </label>
                              <label className="moderation-field">
                                Proof Note
                                <textarea
                                  value={getExecutionNote(entry)}
                                  onChange={(event) =>
                                    updateExecutionEntry(
                                      index,
                                      "status_proof_note",
                                      event.target.value
                                    )
                                  }
                                  maxLength={MAX_NOTE_CHARS}
                                  rows={3}
                                />
                              </label>
                            </div>
                          ))}
                        </div>
                      </>
                    ) : null}

                    <label className="moderation-field">
                      Update Note
                      <textarea
                        value={executionUpdateNote}
                        onChange={(event) => setExecutionUpdateNote(event.target.value)}
                        maxLength={MAX_NOTE_CHARS}
                        rows={3}
                      />
                    </label>

                    {executionUpdateError ? (
                      <div className="error-box">{executionUpdateError}</div>
                    ) : null}
                    {executionUpdateSuccess ? (
                      <div className="success-box">{executionUpdateSuccess}</div>
                    ) : null}

                    <button type="submit" disabled={executionUpdateLoading}>
                      {executionUpdateLoading ? "Saving..." : "Save Implementation Update"}
                    </button>
                  </form>
                ) : null}
              </div>
            ) : null}

            {selectedProposal?.proposal ? (
              <div className="detail-card detail-card-drawer">
                {selectedIsSolutionTargetIssue ? (
                  <div className="solution-target-context">
                    <span className="state-pill solution-target-pill">
                      Problem Being Solved
                    </span>
                    <p className="muted">
                      This is the issue that the current Solutions board is trying to solve.
                    </p>
                  </div>
                ) : null}
                <h3>{selectedProposal.proposal.title}</h3>

                <div className="detail-grid">
                  {isModerator ? (
                    <div>
                      <strong>State:</strong> {selectedStateLabel}
                    </div>
                  ) : null}
                  <div>
                    <strong>Board:</strong> {selectedProposal.proposal.board_code}
                  </div>
                  <div>
                    <strong>Author:</strong> {selectedProposal.proposal.author_user_id}
                  </div>
                  <div>
                    <strong>Created:</strong> {selectedProposal.proposal.created_at}
                  </div>
                </div>

                {selectedProposal.proposal.archived_reason ? (
                  <>
                    <h4>Archived Reason</h4>
                    <p>{selectedProposal.proposal.archived_reason}</p>
                  </>
                ) : null}

                {selectedProposal.proposal.moderation_note ? (
                  <>
                    <h4>Moderation Note</h4>
                    <p>{selectedProposal.proposal.moderation_note}</p>
                  </>
                ) : null}

                {showUseArchivedAsDraft ? (
                  <div className="moderation-box">
                    <h4>Re-Propose</h4>
                    <p className="muted">
                      Copy this archived proposal into a new current-cycle draft.
                    </p>
                    <button type="button" onClick={handleUseArchivedProposalAsDraft}>
                      Use as New {formatActionType(selectedBoardCode)} Draft
                    </button>
                  </div>
                ) : null}

                {selectedIsArchived && selectedArchiveRestoreBlocked ? (
                  <p className="muted">
                    This archived record is historical and cannot be restored through the
                    normal appeal or reconsideration path.
                  </p>
                ) : null}

                {showAuthorAppeal ? (
                  <form className="moderation-box" onSubmit={handleSubmitAppeal}>
                    <h4>Appeal Archive</h4>

                    <label className="moderation-field">
                      Reason
                      <textarea
                        value={appealReason}
                        onChange={(event) => setAppealReason(event.target.value)}
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                        required
                      />
                    </label>

                    <label className="moderation-field">
                      Clarification
                      <textarea
                        value={appealClarification}
                        onChange={(event) => setAppealClarification(event.target.value)}
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                      />
                    </label>

                    {appealError ? <div className="error-box">{appealError}</div> : null}
                    {appealSuccess ? (
                      <div className="success-box">{appealSuccess}</div>
                    ) : null}

                    <button type="submit" disabled={appealLoading}>
                      {appealLoading ? "Submitting..." : "Submit Appeal"}
                    </button>
                  </form>
                ) : null}

                {showAppealReviewControls ? (
                  <div className="moderation-box">
                    <h4>Appeal Review</h4>

                    <div className="detail-grid">
                      <div>
                        <strong>Status:</strong> {selectedAppeal.status}
                      </div>
                      <div>
                        <strong>Submitted:</strong> {selectedAppeal.created_at}
                      </div>
                    </div>

                    <h4>Appeal Reason</h4>
                    <p>{selectedAppeal.appeal_reason}</p>

                    {selectedAppeal.clarification_note ? (
                      <>
                        <h4>Clarification</h4>
                        <p>{selectedAppeal.clarification_note}</p>
                      </>
                    ) : null}

                    {selectedAppealMustRecuse ? (
                      <div className="error-box">
                        A different moderator must resolve this appeal when another
                        moderator is available.
                      </div>
                    ) : null}
                    {selectedAppealRestoreBlocked ? (
                      <div className="error-box">
                        This archived record cannot be restored through the normal
                        appeal path in v1.
                      </div>
                    ) : null}

                    <label className="moderation-field">
                      Moderator Note
                      <textarea
                        value={appealResolveNote}
                        onChange={(event) => setAppealResolveNote(event.target.value)}
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                        required
                      />
                    </label>

                    {appealResolveError ? (
                      <div className="error-box">{appealResolveError}</div>
                    ) : null}
                    {appealResolveSuccess ? (
                      <div className="success-box">{appealResolveSuccess}</div>
                    ) : null}

                    <div className="action-row">
                      <button
                        type="button"
                        onClick={() => handleResolveAppeal("restore")}
                        disabled={
                          appealResolveLoading ||
                          selectedAppeal.status !== "pending" ||
                          selectedAppealMustRecuse ||
                          selectedAppealRestoreBlocked ||
                          !appealResolveNote.trim()
                        }
                      >
                        {appealResolveLoading ? "Resolving..." : "Restore Proposal"}
                      </button>
                      <button
                        type="button"
                        className="danger-button"
                        onClick={() => handleResolveAppeal("uphold_archive")}
                        disabled={
                          appealResolveLoading ||
                          selectedAppeal.status !== "pending" ||
                          selectedAppealMustRecuse ||
                          !appealResolveNote.trim()
                        }
                      >
                        Uphold Archive
                      </button>
                    </div>
                  </div>
                ) : null}

                {showStartReconsideration ? (
                  <form className="moderation-box" onSubmit={handleStartReconsideration}>
                    <h4>Start Reconsideration</h4>

                    <label className="moderation-field">
                      Reason
                      <textarea
                        value={reconsiderationReason}
                        onChange={(event) =>
                          setReconsiderationReason(event.target.value)
                        }
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                        required
                      />
                    </label>

                    <label className="moderation-field">
                      Note
                      <textarea
                        value={reconsiderationNote}
                        onChange={(event) =>
                          setReconsiderationNote(event.target.value)
                        }
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                      />
                    </label>

                    {reconsiderationError ? (
                      <div className="error-box">{reconsiderationError}</div>
                    ) : null}
                    {reconsiderationSuccess ? (
                      <div className="success-box">{reconsiderationSuccess}</div>
                    ) : null}

                    <button type="submit" disabled={reconsiderationLoading}>
                      {reconsiderationLoading ? "Opening..." : "Open 72h Window"}
                    </button>
                  </form>
                ) : null}

                {showReconsiderationReviewControls ? (
                  <div className="moderation-box">
                    <h4>Reconsideration Review</h4>

                    <div className="detail-grid">
                      <div>
                        <strong>Status:</strong> {selectedReconsideration.status}
                      </div>
                      <div>
                        <strong>Due:</strong>{" "}
                        {selectedReconsideration.review_due ? "Yes" : "No"}
                      </div>
                      <div>
                        <strong>Started:</strong> {selectedReconsideration.starts_at}
                      </div>
                      <div>
                        <strong>Ends:</strong> {selectedReconsideration.ends_at}
                      </div>
                    </div>

                    <h4>Reason</h4>
                    <p>{selectedReconsideration.start_reason}</p>

                    {selectedReconsideration.start_note ? (
                      <>
                        <h4>Note</h4>
                        <p>{selectedReconsideration.start_note}</p>
                      </>
                    ) : null}
                    {selectedReconsiderationRestoreBlocked ? (
                      <div className="error-box">
                        This archived record cannot be restored through the normal
                        reconsideration path in v1.
                      </div>
                    ) : null}

                    <label className="moderation-field">
                      Resolution Note
                      <textarea
                        value={reconsiderationResolveNote}
                        onChange={(event) =>
                          setReconsiderationResolveNote(event.target.value)
                        }
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                      />
                    </label>

                    {reconsiderationResolveError ? (
                      <div className="error-box">{reconsiderationResolveError}</div>
                    ) : null}
                    {reconsiderationResolveSuccess ? (
                      <div className="success-box">{reconsiderationResolveSuccess}</div>
                    ) : null}

                    <div className="action-row">
                      <button
                        type="button"
                        onClick={() => handleResolveReconsideration("restore_active")}
                        disabled={
                          reconsiderationResolveLoading ||
                          selectedReconsiderationRestoreBlocked ||
                          !selectedReconsideration.review_due
                        }
                      >
                        Restore Active
                      </button>
                      <button
                        type="button"
                        onClick={() => handleResolveReconsideration("return_archive")}
                        disabled={
                          reconsiderationResolveLoading ||
                          !selectedReconsideration.review_due
                        }
                      >
                        Return Archive
                      </button>
                      <button
                        type="button"
                        className="danger-button"
                        onClick={() => handleResolveReconsideration("freeze")}
                        disabled={
                          reconsiderationResolveLoading ||
                          selectedReconsiderationRestoreBlocked ||
                          !selectedReconsideration.review_due
                        }
                      >
                        Freeze
                      </button>
                    </div>
                  </div>
                ) : null}

                {selectedProposal.proposal.problem_description ? (
                  <>
                    <h4>Problem Description</h4>
                    <p>{selectedProposal.proposal.problem_description}</p>
                  </>
                ) : null}

                {selectedProposal.proposal.affected_scope ? (
                  <>
                    <h4>Affected Scope</h4>
                    <p>{selectedProposal.proposal.affected_scope}</p>
                  </>
                ) : null}

                {selectedProposal.proposal.action_description ? (
                  <>
                    <h4>Action Description</h4>
                    <p>{selectedProposal.proposal.action_description}</p>
                  </>
                ) : null}

                {selectedProposal.proposal.why_it_matters ? (
                  <>
                    <h4>
                      {selectedProposal.proposal.board_code === "solution"
                        ? "Why This Solves It"
                        : "Why It Matters"}
                    </h4>
                    <p>{selectedProposal.proposal.why_it_matters}</p>
                  </>
                ) : null}

                {asArray(selectedProposal.proposal.required_resource_categories).length ? (
                  <>
                    <h4>Required Resources</h4>
                    <div className="proposal-badge-stack">
                      {asArray(
                        selectedProposal.proposal.required_resource_categories
                      ).map((category) => (
                        <span className="state-pill subtle-pill" key={category}>
                          {formatResourceCategory(category)}
                        </span>
                      ))}
                    </div>
                  </>
                ) : null}

                {asArray(selectedProposal.proposal.completion_criteria).length ? (
                  <>
                    <h4>Completion Criteria</h4>
                    <div className="relationship-block">
                      {asArray(selectedProposal.proposal.completion_criteria).map(
                        (criterion, index) => (
                          <div className="relationship-card" key={`criterion-${index}`}>
                            <strong>{getCriterionDescription(criterion)}</strong>
                            {getCriterionStatus(criterion) ? (
                              <p className="muted">
                                {formatActionType(getCriterionStatus(criterion))}
                              </p>
                            ) : null}
                            {getCriterionEvidence(criterion) ? (
                              <p>{getCriterionEvidence(criterion)}</p>
                            ) : null}
                          </div>
                        )
                      )}
                    </div>
                  </>
                ) : null}

                {asArray(selectedProposal.proposal.execution_tracking_entries).length ? (
                  <>
                    <h4>Required Resources</h4>
                    <div className="relationship-block">
                      {asArray(
                        selectedProposal.proposal.execution_tracking_entries
                      ).map((entry, index) => (
                        <div className="relationship-card" key={`execution-${index}`}>
                          <strong>
                            {formatResourceCategory(entry?.resource_category || "other")}
                          </strong>
                          <p>
                            <strong>Needed:</strong> {entry?.target_needed || "Not specified"}
                          </p>
                          {entry?.current_acquired_amount ? (
                            <p>
                              <strong>Acquired:</strong> {entry.current_acquired_amount}
                            </p>
                          ) : null}
                          {entry?.external_coordination_link ? (
                            <p>
                              <a
                                href={entry.external_coordination_link}
                                target="_blank"
                                rel="noreferrer"
                              >
                                Coordination Link
                              </a>
                            </p>
                          ) : null}
                          {getExecutionNote(entry) ? <p>{getExecutionNote(entry)}</p> : null}
                        </div>
                      ))}
                    </div>
                  </>
                ) : null}

                {personalVotingPanel}

                {showExecutionRecordControls ? (
                  <div className="moderation-box">
                    <h4>Implementation Record</h4>

                    <button
                      type="button"
                      onClick={handleCreateExecutionRecord}
                      disabled={executionLoading}
                    >
                      {executionLoading ? "Creating..." : "Create Implementation Record"}
                    </button>

                    {executionError ? (
                      <div className="error-box">{executionError}</div>
                    ) : null}
                    {executionSuccess ? (
                      <div className="success-box">{executionSuccess}</div>
                    ) : null}
                  </div>
                ) : null}

                {isModerator ? (
                  <>
                    <h4>Vote Totals</h4>
                    <div className="proposal-counts">
                      <span>Support: {selectedProposal.proposal.support_count}</span>
                      <span>Not fit: {selectedProposal.proposal.not_a_fit_count}</span>
                      <span>Unclear: {selectedProposal.proposal.unclear_count}</span>
                      <span>Unsafe: {selectedProposal.proposal.unsafe_count}</span>
                      <span>Merge: {selectedProposal.proposal.merge_count}</span>
                    </div>
                  </>
                ) : null}

                {showDistinctionForm ? (
                  <form className="moderation-box" onSubmit={handleSaveDistinctionNote}>
                    <h4>Distinction Note</h4>

                    <label className="moderation-field">
                      Related Proposal
                      <select
                        value={distinctionTargetId}
                        onChange={(event) =>
                          handleDistinctionTargetChange(event.target.value)
                        }
                        required
                      >
                        {selectedOutgoingRelationships.map((rel) => (
                          <option
                            key={rel.target_proposal_id}
                            value={rel.target_proposal_id}
                          >
                            {rel.target_title}
                          </option>
                        ))}
                      </select>
                    </label>

                    <label className="moderation-field">
                      Difference Type
                      <select
                        value={distinctionType}
                        onChange={(event) => setDistinctionType(event.target.value)}
                      >
                        {DIFFERENCE_TYPE_OPTIONS.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </label>

                    <label className="moderation-field">
                      Explanation
                      <textarea
                        value={distinctionText}
                        onChange={(event) => setDistinctionText(event.target.value)}
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                        required
                      />
                    </label>

                    {distinctionError ? (
                      <div className="error-box">{distinctionError}</div>
                    ) : null}
                    {distinctionSuccess ? (
                      <div className="success-box">{distinctionSuccess}</div>
                    ) : null}

                    <button type="submit" disabled={distinctionLoading}>
                      {distinctionLoading ? "Saving..." : "Save Distinction Note"}
                    </button>
                  </form>
                ) : null}
                {showDistinctionLocked ? (
                  <div className="moderation-box">
                    <h4>Distinction Note</h4>
                    <p className="muted">
                      This relationship can receive a distinction note after the
                      proposal receives enough duplicate signals.
                    </p>
                  </div>
                ) : null}

                {showMergeControls ? (
                  <div className="moderation-box">
                    <h4>Merge Action</h4>

                    <label className="moderation-field">
                      Related Proposal
                      <select
                        value={mergeTargetId}
                        onChange={(event) => setMergeTargetId(event.target.value)}
                      >
                        <option value="">Select a related proposal</option>
                        {mergeOptions.map((option) => (
                          <option key={option.id} value={option.id}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </label>

                    {mergeTargetId ? (
                      <div className="compact-card">
                        <strong>Merge Direction</strong>
                        {mergeDirectionKnown && !mergeTotalsEqual ? (
                          <>
                            <p>
                              Archive {mergeArchiveProposal?.title} ({mergeArchiveTotal} votes)
                              into {mergeSurvivingProposal?.title} ({mergeSurvivingTotal} votes).
                            </p>
                            {mergeBlockedByThreshold ? (
                              <p>
                                Merge is blocked until either proposal receives enough
                                pair-specific duplicate signals with the other proposal
                                selected as its target.
                              </p>
                            ) : null}
                            {mergePairThresholdKnown && mergePairThresholdMet ? (
                              <p className="muted">
                                Pair-specific merge threshold is met by{" "}
                                {mergeThresholdDirections}.
                              </p>
                            ) : null}
                            {!mergePairThresholdKnown ? (
                              <p className="muted">
                                Pair-specific merge threshold will be verified when the
                                merge is executed.
                              </p>
                            ) : null}
                            <p className="muted">
                              Sentiment votes transfer only when that voter has not voted on
                              the survivor. Duplicate or conflicting sentiment votes are
                              discarded and logged.
                            </p>
                          </>
                        ) : null}
                        {mergeTotalsEqual ? (
                          <p>
                            Merge is blocked because both proposals currently have the same
                            total vote count.
                          </p>
                        ) : null}
                        {!mergeDirectionKnown ? (
                          <p>
                            The API will archive the lower total-count proposal and keep the
                            higher total-count proposal.
                          </p>
                        ) : null}
                      </div>
                    ) : null}

                    {mergeError ? <div className="error-box">{mergeError}</div> : null}
                    {mergeSuccess ? <div className="success-box">{mergeSuccess}</div> : null}

                    <button
                      type="button"
                      className="danger-button"
                      onClick={handleExecuteMerge}
                      disabled={
                        mergeLoading ||
                        !mergeTargetId ||
                        mergeTotalsEqual ||
                        mergeBlockedByThreshold
                      }
                    >
                      {mergeLoading ? "Executing merge..." : "Merge Lower Into Higher"}
                    </button>
                  </div>
                ) : null}

                {showModerationControls ? (
                  <div className="moderation-box">
                    <h4>Moderation Action</h4>

                    <label className="moderation-field">
                      Archive Reason
                      <select
                        value={moderationReason}
                        onChange={(event) => setModerationReason(event.target.value)}
                      >
                        <option value="irrelevant">irrelevant</option>
                        <option value="not_a_fit">not_a_fit</option>
                        <option value="moderation">moderation</option>
                        <option value="manual_archive">manual_archive</option>
                      </select>
                    </label>

                    <label className="moderation-field">
                      Moderation Note
                      <textarea
                        value={moderationNote}
                        onChange={(event) => setModerationNote(event.target.value)}
                        maxLength={MAX_NOTE_CHARS}
                        rows={4}
                        placeholder="Add a short explanation for the moderation action."
                      />
                    </label>

                    {moderationError ? (
                      <div className="error-box">{moderationError}</div>
                    ) : null}

                    {moderationSuccess ? (
                      <div className="success-box">{moderationSuccess}</div>
                    ) : null}

                    <div className="action-row">
                      <button
                        type="button"
                        onClick={handleModerateReviewedActive}
                        disabled={moderationLoading || !moderationNote.trim()}
                      >
                        {moderationLoading ? "Recording..." : "Mark Reviewed Active"}
                      </button>

                      {!selectedIsFrozen ? (
                        <button
                          type="button"
                          onClick={handleModerateFreeze}
                          disabled={moderationLoading}
                        >
                          {moderationLoading ? "Freezing..." : "Freeze for Review"}
                        </button>
                      ) : (
                        <button
                          type="button"
                          onClick={handleModerateUnfreeze}
                          disabled={moderationLoading}
                        >
                          {moderationLoading ? "Unfreezing..." : "Unfreeze"}
                        </button>
                      )}

                      <button
                        type="button"
                        className="danger-button"
                        onClick={handleModerateArchive}
                        disabled={moderationLoading}
                      >
                        {moderationLoading ? "Archiving..." : "Archive Proposal"}
                      </button>
                    </div>
                  </div>
                ) : null}

                {showModerationObservationOnly ? (
                  <div className="moderation-box">
                    <h4>Moderator Observation</h4>
                    <p className="muted">
                      This proposal is visible for moderator awareness, but it has not
                      reached the moderation action threshold and is not frozen.
                    </p>
                  </div>
                ) : null}

                <details className="more-info-panel">
                  <summary>More info</summary>

                  <h4>Audit Trail</h4>
                  {selectedProposal.moderator_actions?.length ? (
                    <div className="relationship-block">
                      {selectedProposal.moderator_actions.map((action) => (
                        <div className="relationship-card" key={action.id}>
                          <div>
                            <strong>{formatActionType(action.action_type)}</strong>
                          </div>
                          <p className="muted">{action.created_at}</p>
                          {action.action_reason ? (
                            <p>
                              <strong>Reason:</strong> {action.action_reason}
                            </p>
                          ) : null}
                          {action.related_proposal_title ? (
                            <p>
                              <strong>Related:</strong> {action.related_proposal_title}
                            </p>
                          ) : null}
                          {action.public_note ? (
                            <p className="relationship-note">{action.public_note}</p>
                          ) : null}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="muted">No moderator actions recorded.</p>
                  )}

                  <h4>Merge Relationships</h4>

                  <div className="relationship-block">
                    <strong>Outgoing</strong>
                    {selectedProposal.merge_relationships?.outgoing?.length ? (
                      selectedProposal.merge_relationships.outgoing.map((rel) => (
                        <div
                          className="relationship-card"
                          key={`${rel.source_proposal_id}-${rel.target_proposal_id}`}
                        >
                          <div>
                            <strong>Target:</strong> {rel.target_title}
                          </div>
                          <p className="muted small-muted">
                            ID: {rel.target_proposal_id}
                          </p>
                          {rel.note ? (
                            <>
                              <p className="muted">
                                {formatActionType(rel.note.difference_type)}
                              </p>
                              <p className="relationship-note">{rel.note.note_text}</p>
                            </>
                          ) : (
                            <p className="muted">No distinction note.</p>
                          )}
                        </div>
                      ))
                    ) : (
                      <p className="muted">No outgoing merge relationships.</p>
                    )}
                  </div>

                  <div className="relationship-block">
                    <strong>Incoming</strong>
                    {selectedProposal.merge_relationships?.incoming?.length ? (
                      selectedProposal.merge_relationships.incoming.map((rel) => (
                        <div
                          className="relationship-card"
                          key={`${rel.source_proposal_id}-${rel.target_proposal_id}`}
                        >
                          <div>
                            <strong>Source:</strong> {rel.source_title}
                          </div>
                          <p className="muted small-muted">
                            ID: {rel.source_proposal_id}
                          </p>
                          {rel.note ? (
                            <>
                              <p className="muted">
                                {formatActionType(rel.note.difference_type)}
                              </p>
                              <p className="relationship-note">{rel.note.note_text}</p>
                            </>
                          ) : (
                            <p className="muted">No distinction note.</p>
                          )}
                        </div>
                      ))
                    ) : (
                      <p className="muted">No incoming merge relationships.</p>
                    )}
                  </div>
                </details>
              </div>
            ) : null}
          </div>
        </div>

        <div className="bottom-dock" style={{ "--dock-columns": dockColumnCount }}>
          <button
            type="button"
            className={`dock-button ${navDockActive ? "dock-button-active" : ""}`}
            onClick={toggleNavDrawer}
            disabled={navigationTabs.length === 0}
            aria-pressed={navDockActive}
          >
            <span className="dock-icon" aria-hidden="true">☰</span>
            <span className="dock-label">Sections</span>
          </button>

          {showSubmissionDockButton ? (
            <button
              type="button"
              className={`dock-button submission-dock-button ${
                submissionDockActive ? "dock-button-active" : ""
              }`}
              onClick={handleSubmissionButton}
              disabled={!currentUserSubmission && !canSubmitOnCurrentBoard}
              aria-pressed={submissionDockActive}
            >
              <span className="dock-icon" aria-hidden="true">+</span>
              <span className="dock-label">{submissionDockLabel}</span>
            </button>
          ) : null}

          <button
            type="button"
            className={`dock-button ${detailDockActive ? "dock-button-active" : ""}`}
            onClick={toggleDetailDrawer}
            aria-pressed={detailDockActive}
          >
            <span className="dock-icon" aria-hidden="true">⌄</span>
            <span className="dock-label">{detailDockLabel}</span>
          </button>
        </div>
      </section>
    </div>
  );
}

export default App;
