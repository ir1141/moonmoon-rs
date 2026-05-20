export function chatLoadStatusText() {
  return "Loading chat...";
}

export function chatEmptyStatusText() {
  return "No chat at this timestamp";
}

export function chatErrorStatusText() {
  return "Chat unavailable";
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
