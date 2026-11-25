import { computed, nextTick, ref, watch, type Ref } from "vue";
import type { CheckState } from "../../../shared/types";
import type { BridgeRequest, BridgeSummary, PortAllocation, remoteBackend, SshAuthOperation, SshAuthSnapshot } from "../state";

export type ConnectionPhase = "idle" | "checking" | "authenticating" | "building" | "succeeded" | "failed";

interface RemoteInteractiveAuthOptions {
  backend: Pick<typeof remoteBackend, "sshAuthBegin" | "sshAuthState" | "sshAuthConfirmHost" | "sshAuthSubmit" | "sshAuthFinish" | "sshAuthCancel">;
  targetId: Readonly<Ref<string>>;
  connectionPhase: Ref<ConnectionPhase>;
  busy: Ref<boolean>;
  checked: Ref<boolean>;
  sshCheck: Ref<{ state: CheckState; checkedAt: number | null }>;
  error: Ref<unknown>;
  readinessAnnouncementTarget: Ref<string | undefined>;
  dialog: {
    isOpen: () => boolean | undefined;
    show: () => void;
    close: () => void;
    focusPrompt: () => void;
    focusHeading: () => void;
  };
  usePorts: (ports: PortAllocation) => void;
  request: () => BridgeRequest;
  connectPrepared: (request: BridgeRequest, isCurrent: () => boolean) => Promise<BridgeSummary | undefined>;
  finishConnectionUi: (summary: BridgeSummary) => Promise<void>;
  isInteractiveAuthError: (cause: unknown) => boolean;
  invalidateNetworkChecks: () => void;
  refresh: () => void;
}

export function useRemoteInteractiveAuth({
  backend, targetId, connectionPhase, busy, checked, sshCheck, error, readinessAnnouncementTarget, dialog,
  usePorts, request, connectPrepared, finishConnectionUi, isInteractiveAuthError, invalidateNetworkChecks, refresh,
}: RemoteInteractiveAuthOptions) {
  const authSession = ref<SshAuthSnapshot>();
  const lastAuthPrompt = ref<SshAuthSnapshot["prompt"]>(null);
  const authResponse = ref("");
  const submittedAuthMask = ref("");
  const submittedAuthPromptId = ref<string>();
  const authSubmitting = ref(false);
  const showAuthDiagnostic = ref(false);
  const authRetryContext = ref<{ operation: SshAuthOperation; request: BridgeRequest | null }>();
  let authBeginRevision = 0;
  let authPollTimer: ReturnType<typeof setTimeout> | undefined;

  const authPrompt = computed(() => authSession.value?.prompt);
  const visibleAuthPrompt = computed(() => authPrompt.value ?? lastAuthPrompt.value);
  const authCanRespond = computed(() => authSession.value?.status === "waitingUser" && !!authPrompt.value);
  const authIsHostConfirmation = computed(() => visibleAuthPrompt.value?.type === "hostKeyConfirmation");
  const authInputValue = computed(() => authSubmitting.value || !authCanRespond.value ? submittedAuthMask.value : authResponse.value);

  function advanceAuthRevision() {
    return ++authBeginRevision;
  }

  function isCurrentAuthRevision(revision: number) {
    return revision === authBeginRevision;
  }

  function clearAuthPoll() {
    clearTimeout(authPollTimer);
    authPollTimer = undefined;
  }

  async function completeInteractiveAuth(snapshot: SshAuthSnapshot) {
    const revision = authBeginRevision;
    const target = targetId.value;
    const isCurrent = () => revision === authBeginRevision
      && authSession.value?.sessionId === snapshot.sessionId && targetId.value === target;
    clearAuthPoll();
    authSession.value = snapshot;
    connectionPhase.value = "building";
    const outcome = await backend.sshAuthFinish(snapshot.sessionId);
    if (!isCurrent()) return;
    if (outcome.operation === "check" && outcome.ports) {
      usePorts(outcome.ports);
      checked.value = true;
      sshCheck.value = { state: "healthy", checkedAt: Date.now() };
      connectionPhase.value = "building";
      try {
        const summary = await connectPrepared(request(), isCurrent);
        if (!isCurrent() || !summary) return;
        await finishConnectionUi(summary);
      } catch (cause) {
        if (!isCurrent()) return;
        if (isInteractiveAuthError(cause)) {
          await beginInteractiveAuth("connect", request());
          return;
        }
        connectionPhase.value = "failed";
        readinessAnnouncementTarget.value = undefined;
        error.value = cause;
        return;
      }
    } else if (outcome.operation === "connect" && outcome.summary) {
      await finishConnectionUi(outcome.summary);
    }
    if (revision === authBeginRevision) refresh();
  }

  async function pollInteractiveAuth() {
    const session = authSession.value;
    if (!session) return;
    const revision = authBeginRevision;
    try {
      const next = await backend.sshAuthState(session.sessionId);
      if (revision !== authBeginRevision || authSession.value?.sessionId !== session.sessionId) return;
      authSession.value = next;
      if (next.status === "succeeded") {
        await completeInteractiveAuth(next);
        return;
      }
      if (next.status === "failed") {
        connectionPhase.value = "failed";
        readinessAnnouncementTarget.value = undefined;
        busy.value = false;
        clearAuthPoll();
        refresh();
        return;
      }
    } catch (cause) {
      if (revision !== authBeginRevision) return;
      connectionPhase.value = "failed";
      readinessAnnouncementTarget.value = undefined;
      sshCheck.value = { state: "failed", checkedAt: Date.now() };
      busy.value = false;
      error.value = cause;
      clearAuthPoll();
      refresh();
      return;
    }
    if (!["failed", "promptUnavailable"].includes(authSession.value?.status ?? "")) {
      authPollTimer = setTimeout(pollInteractiveAuth, 180);
    }
  }

  async function beginInteractiveAuth(operation: SshAuthOperation, request: BridgeRequest | null = null) {
    clearAuthPoll();
    const revision = advanceAuthRevision();
    const continuing = operation === "connect" && dialog.isOpen();
    error.value = undefined;
    authSession.value = undefined;
    authResponse.value = "";
    if (!continuing) {
      lastAuthPrompt.value = null;
      submittedAuthMask.value = "";
      submittedAuthPromptId.value = undefined;
    }
    showAuthDiagnostic.value = false;
    authRetryContext.value = { operation, request };
    connectionPhase.value = "authenticating";
    await nextTick();
    if (revision !== authBeginRevision) return;
    if (!dialog.isOpen()) dialog.show();
    let session: SshAuthSnapshot;
    try {
      session = await backend.sshAuthBegin(operation, targetId.value, request);
    } catch (cause) {
      if (revision === authBeginRevision) {
        dialog.close();
        connectionPhase.value = "failed";
        readinessAnnouncementTarget.value = undefined;
        error.value = cause;
      }
      throw cause;
    }
    if (revision !== authBeginRevision || connectionPhase.value !== "authenticating") {
      await backend.sshAuthCancel(session.sessionId).catch(() => undefined);
      return;
    }
    authSession.value = session;
    authPollTimer = setTimeout(pollInteractiveAuth, 120);
  }

  async function submitInteractiveAuth() {
    const session = authSession.value;
    const promptId = session?.prompt?.id;
    if (!session || !promptId || !authCanRespond.value || authSubmitting.value) return;
    if (!authIsHostConfirmation.value && !authResponse.value) return;
    error.value = undefined;
    authSubmitting.value = true;
    const revision = authBeginRevision;
    const response = authResponse.value;
    submittedAuthMask.value = visibleAuthPrompt.value?.secret ? "••••••••" : "";
    submittedAuthPromptId.value = promptId;
    authResponse.value = "";
    try {
      const next = authIsHostConfirmation.value
        ? await backend.sshAuthConfirmHost(session.sessionId, promptId)
        : await backend.sshAuthSubmit(session.sessionId, promptId, response);
      if (isCurrentAuthRevision(revision) && authSession.value?.sessionId === session.sessionId) authSession.value = next;
    } catch (cause) {
      if (isCurrentAuthRevision(revision)) error.value = cause;
    } finally {
      if (isCurrentAuthRevision(revision)) authSubmitting.value = false;
    }
  }

  async function retryInteractiveAuth() {
    const retry = authRetryContext.value;
    const sessionId = authSession.value?.sessionId;
    if (!retry || authSubmitting.value) return;
    authSubmitting.value = true;
    let revision = authBeginRevision;
    readinessAnnouncementTarget.value = targetId.value;
    clearAuthPoll();
    try {
      if (sessionId) {
        try { await backend.sshAuthCancel(sessionId); } catch { /* The timed-out PTY may already be closed. */ }
      }
      if (revision !== authBeginRevision) return;
      const pending = beginInteractiveAuth(retry.operation, retry.request);
      revision = authBeginRevision;
      await pending;
    } catch (cause) {
      if (revision === authBeginRevision) error.value = cause;
    } finally {
      if (revision === authBeginRevision) authSubmitting.value = false;
    }
  }

  watch(
    // Watch primitive values separately: identical polling snapshots must not steal text selection.
    [
      () => authSession.value?.prompt?.id,
      () => authSession.value?.prompt?.type,
      () => authSession.value?.prompt?.attempt,
      () => authSession.value?.status,
    ],
    async () => {
      if (authPrompt.value) {
        if (authPrompt.value.id !== submittedAuthPromptId.value) {
          submittedAuthMask.value = "";
          submittedAuthPromptId.value = undefined;
        }
        lastAuthPrompt.value = authPrompt.value;
      }
      if (!authCanRespond.value || authIsHostConfirmation.value) return;
      await nextTick();
      dialog.focusPrompt();
    },
  );

  async function cancelInteractiveAuth() {
    clearAuthPoll();
    const revision = advanceAuthRevision();
    readinessAnnouncementTarget.value = undefined;
    invalidateNetworkChecks();
    busy.value = false;
    authSubmitting.value = false;
    const sessionId = authSession.value?.sessionId;
    clearAuthSession();
    showAuthDiagnostic.value = false;
    connectionPhase.value = "idle";
    dialog.close();
    if (sessionId) {
      try { await backend.sshAuthCancel(sessionId); } catch { /* Session may already be closed. */ }
    }
    if (revision !== authBeginRevision) return;
    if (sshCheck.value.state === "checking") {
      sshCheck.value = { state: "idle", checkedAt: null };
    }
    await nextTick();
    if (connectionPhase.value === "idle") dialog.focusHeading();
  }

  function clearAuthSession() {
    authSession.value = undefined;
    lastAuthPrompt.value = null;
    authResponse.value = "";
    submittedAuthMask.value = "";
    submittedAuthPromptId.value = undefined;
    authRetryContext.value = undefined;
  }

  watch(() => targetId.value, () => {
    if (["checking", "authenticating", "building"].includes(connectionPhase.value)) {
      void cancelInteractiveAuth();
    }
  }, { flush: "sync" });

  function prepareAuthPreview(snapshot: SshAuthSnapshot) {
    authSession.value = snapshot;
    authRetryContext.value = { operation: "check", request: null };
  }

  function detachInteractiveAuth() {
    advanceAuthRevision();
    const sessionId = authSession.value?.sessionId;
    clearAuthPoll();
    clearAuthSession();
    return sessionId;
  }

  return {
    authSession, visibleAuthPrompt, authResponse, authSubmitting, authCanRespond, authIsHostConfirmation,
    authInputValue, showAuthDiagnostic, advanceAuthRevision, isCurrentAuthRevision, beginInteractiveAuth,
    submitInteractiveAuth, retryInteractiveAuth, cancelInteractiveAuth, clearAuthSession, prepareAuthPreview, detachInteractiveAuth,
  };
}
