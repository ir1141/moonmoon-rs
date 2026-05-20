import { pruneEmptyListParameters as pruneEmptyParameters } from "./lib/list-feedback.js";

function listRegionFromEvent(event) {
  const detailTarget = event.detail && event.detail.target;
  const target =
    detailTarget instanceof Element
      ? detailTarget
      : event.target instanceof Element
        ? event.target
        : null;

  if (!target) return null;
  return target.matches("[data-list-region]")
    ? target
    : target.closest("[data-list-region]");
}

function clearListError(region) {
  region.querySelectorAll(".list-error").forEach((error) => error.remove());
}

function setListBusyFromEvent(event) {
  const region = listRegionFromEvent(event);
  if (!region) return;

  clearListError(region);
  region.setAttribute("aria-busy", "true");
}

function clearListBusy() {
  document
    .querySelectorAll('[data-list-region][aria-busy="true"]')
    .forEach((region) => {
      region.setAttribute("aria-busy", "false");
    });
}

function showListError(event) {
  clearListBusy();

  const region = listRegionFromEvent(event);
  if (!region) return;

  clearListError(region);

  const error = document.createElement("div");
  error.className = "list-error";
  error.setAttribute("role", "alert");
  error.textContent = "Could not update results. Check your connection and try again.";

  const loading = region.querySelector(".list-loading");
  if (loading) {
    loading.after(error);
  } else {
    region.prepend(error);
  }
}

function listFilterFormFromEvent(event) {
  const detail = event.detail || {};
  const elt =
    detail.elt instanceof Element
      ? detail.elt
      : event.target instanceof Element
        ? event.target
        : null;
  if (!elt) return null;

  if (elt.matches("[data-list-filters]")) return elt;

  const closest = elt.closest("[data-list-filters]");
  if (closest) return closest;

  const formId = elt.getAttribute("form");
  if (!formId) return null;

  const form = document.getElementById(formId);
  return form && form.matches("[data-list-filters]") ? form : null;
}

function pruneEmptyRequestParameters(event) {
  const form = listFilterFormFromEvent(event);
  if (!form) return;

  pruneEmptyParameters((event.detail || {}).parameters);
}

document.body.addEventListener("htmx:configRequest", pruneEmptyRequestParameters);
document.body.addEventListener("htmx:beforeRequest", setListBusyFromEvent);
document.body.addEventListener("htmx:afterSettle", clearListBusy);
document.body.addEventListener("htmx:responseError", showListError);
document.body.addEventListener("htmx:sendError", showListError);
