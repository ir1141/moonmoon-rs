export function parameterIsEmpty(value) {
  if (Array.isArray(value)) return value.every(parameterIsEmpty);
  return value == null || String(value).trim() === "";
}

export function deleteEmptyParameter(parameters, key) {
  if (parameters instanceof FormData) {
    const values = parameters.getAll(key);
    if (!values.length || values.every(parameterIsEmpty)) parameters.delete(key);
    return;
  }

  if (
    parameters &&
    Object.prototype.hasOwnProperty.call(parameters, key) &&
    parameterIsEmpty(parameters[key])
  ) {
    delete parameters[key];
  }
}

export function pruneEmptyListParameters(parameters) {
  ["search", "from", "to", "page"].forEach((key) => {
    deleteEmptyParameter(parameters, key);
  });
}

/**
 * Label for the mobile filter sheet's dismiss button, derived from the server's
 * own result label ("71 streams", "1 stream", "511 games") so pluralisation
 * stays in one place. The sheet covers the grid it is filtering, so this button
 * is where the count has to appear.
 */
export function overlayApplyState(resultLabel) {
  const match = /^(\d+)\s+(.+)$/.exec(String(resultLabel ?? "").trim());
  if (!match) return { label: "Show results", empty: false };

  const count = Number(match[1]);
  const noun = match[2];
  if (count === 0) return { label: `No ${noun} match`, empty: true };
  return { label: `Show ${count} ${noun}`, empty: false };
}
