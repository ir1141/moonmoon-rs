export const CHAT_SCROLL_PAUSE_THRESHOLD = 100;

export function chatDistanceFromBottom(measurement) {
  var distance =
    measurement.scrollHeight - measurement.scrollTop - measurement.clientHeight;
  return Math.max(0, distance);
}

export function nextChatAutoScrollState(measurement, options) {
  options = options || {};
  var threshold =
    typeof options.threshold === "number"
      ? options.threshold
      : CHAT_SCROLL_PAUSE_THRESHOLD;
  var currentAutoScroll = options.currentAutoScroll !== false;
  var userInitiated = options.userInitiated === true;

  if (!userInitiated) {
    return {
      autoScroll: currentAutoScroll,
      paused: !currentAutoScroll,
    };
  }

  var paused = chatDistanceFromBottom(measurement) > threshold;

  return {
    autoScroll: !paused,
    paused: paused,
  };
}
