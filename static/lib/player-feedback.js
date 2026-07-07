export function chatLoadStatusText() {
  return "Loading chat...";
}

export function chatEmptyStatusText() {
  return "No chat at this timestamp";
}

export function chatErrorStatusText() {
  return "Chat unavailable";
}

// Decides where chat fetch feedback renders: with no messages on screen the
// chat body owns the state (skeleton / empty / error notice); once messages
// are showing, feedback stays compact in the chat header.
export function chatFeedbackView(state, messageCount) {
  const hasMessages = messageCount > 0;

  if (!hasMessages) {
    if (state === "loading" || state === "empty" || state === "error") {
      return { notice: state, headerText: "", headerRetry: false };
    }
    return { notice: null, headerText: "", headerRetry: false };
  }

  if (state === "loading") {
    return { notice: null, headerText: chatLoadStatusText(), headerRetry: false };
  }
  if (state === "error") {
    return { notice: null, headerText: chatErrorStatusText(), headerRetry: true };
  }
  return { notice: null, headerText: "", headerRetry: false };
}

export function playerFallbackText(reason) {
  if (reason === "missing-video") {
    return "No playable YouTube video is available for this stream.";
  }

  return "Player unavailable. The YouTube player could not be initialized.";
}

export function nextPlayerFallbackState(state, action) {
  if (action.type === "show") {
    return {
      shown: true,
      playerHidden: true,
      reason: action.reason || "api-failed",
    };
  }

  if (action.type === "player-ready") {
    return {
      shown: false,
      playerHidden: false,
      reason: null,
    };
  }

  return {
    shown: !!state.shown,
    playerHidden: !!state.playerHidden,
    reason: state.reason || null,
  };
}
