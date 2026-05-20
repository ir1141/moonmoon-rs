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
