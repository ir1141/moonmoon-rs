import { nextSearchOverlayState } from "./lib/header-search.js";

function updateBodyOverlayState() {
  document.body.classList.toggle(
    "search-overlay-open",
    !!document.querySelector("[data-search-overlay].is-search-open"),
  );
}

function dispatchInput(input) {
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function initSearchOverlay(form) {
  if (!(form instanceof HTMLFormElement) || !form.id) return;

  const input = form.querySelector('input[type="search"]');
  const openButton = document.querySelector(
    '[data-search-overlay-open][aria-controls="' + form.id + '"]',
  );
  const closeButton = form.querySelector("[data-search-overlay-close]");
  const clearButton = form.querySelector("[data-search-overlay-clear]");

  if (!(input instanceof HTMLInputElement) || !openButton) return;
  const searchInput = /** @type {HTMLInputElement} */ (input);
  const opener = /** @type {Element} */ (openButton);

  function apply(action) {
    const previousQuery = searchInput.value;
    const state = nextSearchOverlayState(
      {
        open: form.classList.contains("is-search-open"),
        query: searchInput.value,
      },
      action,
    );

    form.classList.toggle("is-search-open", state.open);
    opener.setAttribute("aria-expanded", state.open ? "true" : "false");

    if (searchInput.value !== state.query) {
      searchInput.value = state.query;
      if (previousQuery !== state.query) dispatchInput(searchInput);
    }

    updateBodyOverlayState();

    if (state.focusInput) {
      window.requestAnimationFrame(() => searchInput.focus());
    }
  }

  opener.addEventListener("click", () => apply({ type: "open" }));

  if (closeButton) {
    closeButton.addEventListener("click", () => apply({ type: "close" }));
  }

  if (clearButton) {
    clearButton.addEventListener("click", () => apply({ type: "clear" }));
  }

  form.addEventListener("keydown", (event) => {
    if (event.key === "Escape") apply({ type: "escape" });
  });

  form.addEventListener("click", (event) => {
    apply({ type: "backdrop", onBackdrop: event.target === form });
  });
}

document
  .querySelectorAll("[data-search-overlay]")
  .forEach((form) => initSearchOverlay(form));
