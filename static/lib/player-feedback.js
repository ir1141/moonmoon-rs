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
