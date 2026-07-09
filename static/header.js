import {
  isSearchShortcut,
  nextSearchOverlayState,
  nextTrapTarget,
  shouldLockSearchOverlayScroll,
} from "./lib/header-search.js";
import { nextHeaderHidden } from "./lib/header-scroll.js";

const searchOverlayMedia = window.matchMedia("(max-width: 768px)");

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function updateBodyOverlayState() {
  const open = !!document.querySelector("[data-search-overlay].is-search-open");
  document.body.classList.toggle(
    "search-overlay-open",
    shouldLockSearchOverlayScroll({
      open,
      mobile: searchOverlayMedia.matches,
    }),
  );
}

/**
 * The overlay is a modal dialog only while it is the full-screen mobile sheet;
 * on desktop the same form is an inline toolbar, where those roles would lie.
 */
function syncOverlaySemantics(form) {
  const modal = form.classList.contains("is-search-open") && searchOverlayMedia.matches;
  if (modal) {
    form.setAttribute("role", "dialog");
    form.setAttribute("aria-modal", "true");
    if (form.id) form.setAttribute("aria-labelledby", `${form.id}-title`);
  } else {
    form.removeAttribute("role");
    form.removeAttribute("aria-modal");
    form.removeAttribute("aria-labelledby");
  }
}

function focusableItems(form) {
  return Array.from(form.querySelectorAll(FOCUSABLE)).filter(
    (el) => el instanceof HTMLElement && el.offsetParent !== null,
  );
}

function dispatchInput(input) {
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function initSearchOverlay(form) {
  if (!(form instanceof HTMLFormElement) || !form.id) return;
  if (form.dataset.searchOverlayEnhanced === "true") return;

  const input = form.querySelector('input[type="search"]');
  const openButton = document.querySelector(
    '[data-search-overlay-open][aria-controls="' + form.id + '"]',
  );
  // Two dismiss controls: the header ✕ and the "Show N streams" primary.
  const closeButtons = form.querySelectorAll("[data-search-overlay-close]");
  const clearButton = form.querySelector("[data-search-overlay-clear]");

  if (!(input instanceof HTMLInputElement) || !openButton) return;
  form.dataset.searchOverlayEnhanced = "true";
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
    syncOverlaySemantics(form);

    if (searchInput.value !== state.query) {
      searchInput.value = state.query;
      if (previousQuery !== state.query) dispatchInput(searchInput);
    }

    updateBodyOverlayState();

    if (state.focusInput) {
      window.requestAnimationFrame(() => searchInput.focus());
    } else if (state.focusOpener && opener instanceof HTMLElement) {
      opener.focus();
    }
  }

  opener.addEventListener("click", () => apply({ type: "open" }));

  closeButtons.forEach((button) => {
    button.addEventListener("click", () => apply({ type: "close" }));
  });

  if (clearButton) {
    clearButton.addEventListener("click", () => apply({ type: "clear" }));
  }

  form.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      apply({ type: "escape" });
      return;
    }
    if (event.key !== "Tab") return;
    if (!form.classList.contains("is-search-open") || !searchOverlayMedia.matches) return;
    const target = nextTrapTarget(focusableItems(form), document.activeElement, event.shiftKey);
    if (target instanceof HTMLElement) {
      event.preventDefault();
      target.focus();
    }
  });

  form.addEventListener("click", (event) => {
    apply({ type: "backdrop", onBackdrop: event.target === form });
  });
}

function openerFor(form) {
  return document.querySelector(`[data-search-overlay-open][aria-controls="${form.id}"]`);
}

function focusPageSearch() {
  const form = document.querySelector("[data-search-overlay]");
  const input =
    form instanceof HTMLFormElement
      ? form.querySelector('input[type="search"]')
      : document.querySelector('.header input[type="search"]');
  if (!(input instanceof HTMLInputElement)) return false;

  if (
    form instanceof HTMLFormElement &&
    searchOverlayMedia.matches &&
    !form.classList.contains("is-search-open")
  ) {
    const opener = openerFor(form);
    if (opener instanceof HTMLElement) {
      opener.click();
      return true;
    }
  }

  input.focus();
  input.select();
  return true;
}

document.addEventListener("keydown", (event) => {
  if (!isSearchShortcut(event)) return;
  // While a listbox is open its typeahead owns the printable keys.
  if (event.target instanceof Element && event.target.closest('[role="listbox"]')) return;
  if (focusPageSearch()) event.preventDefault();
});

function initSearchOverlays(root) {
  if (root instanceof Element && root.matches("[data-search-overlay]")) {
    initSearchOverlay(root);
  }
  root
    .querySelectorAll("[data-search-overlay]")
    .forEach((form) => initSearchOverlay(form));
}

initSearchOverlays(document);

// htmx swaps replace the toolbar (form and opener together), so rebind the
// fresh nodes and drop the body scroll lock if an open overlay went away.
document.addEventListener("htmx:afterSwap", (event) => {
  if (event.target instanceof Element) initSearchOverlays(event.target);
  updateBodyOverlayState();
});

// Resizing past the breakpoint turns the sheet back into an inline toolbar, so
// the dialog roles and the scroll lock have to go with it.
function onOverlayMediaChange() {
  document.querySelectorAll("[data-search-overlay]").forEach(syncOverlaySemantics);
  updateBodyOverlayState();
}

if (typeof searchOverlayMedia.addEventListener === "function") {
  searchOverlayMedia.addEventListener("change", onOverlayMediaChange);
} else if (typeof searchOverlayMedia.addListener === "function") {
  searchOverlayMedia.addListener(onOverlayMediaChange);
}

function initScrollAwayHeader(header) {
  let lastY = window.scrollY;
  let hidden = false;

  function onScroll() {
    const y = window.scrollY;
    if (!searchOverlayMedia.matches) {
      lastY = y;
      if (hidden) {
        hidden = false;
        header.classList.remove("header-hidden");
      }
      return;
    }
    const next = nextHeaderHidden({
      hidden,
      y,
      lastY,
      headerHeight: header.offsetHeight,
      overlayOpen: document.body.classList.contains("search-overlay-open"),
    });
    if (next !== hidden) {
      hidden = next;
      header.classList.toggle("header-hidden", hidden);
    }
    lastY = y;
  }

  window.addEventListener("scroll", onScroll, { passive: true });
}

const header = document.querySelector(".header");
if (header) initScrollAwayHeader(header);
