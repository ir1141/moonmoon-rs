import {
  datePresetForRange,
  nextSortIndex,
  rangeForDatePreset,
  typeaheadIndex,
} from "./lib/list-filters.js";

const TYPEAHEAD_RESET_MS = 500;

function todayIso() {
  return new Date().toISOString().slice(0, 10);
}

function triggerChange(input) {
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

function setActivePreset(chips, preset) {
  chips.forEach((chip) => {
    chip.classList.toggle("is-active", chip.dataset.preset === preset);
  });
}

function syncDatePresetState(form) {
  const fromInput = form.querySelector('input[name="from"]');
  const toInput = form.querySelector('input[name="to"]');
  const custom = form.querySelector(".date-custom");
  const chips = Array.from(form.querySelectorAll(".date-chip"));
  if (!(fromInput instanceof HTMLInputElement) || !(toInput instanceof HTMLInputElement)) return;
  if (!(custom instanceof HTMLElement) || chips.length === 0) return;

  const preset = datePresetForRange(
    fromInput.value,
    toInput.value,
    todayIso(),
    fromInput.min,
    toInput.max,
  );
  setActivePreset(chips, preset);
  custom.hidden = preset !== "custom";
}

function initDatePresets(form) {
  const fromInput = form.querySelector('input[name="from"]');
  const toInput = form.querySelector('input[name="to"]');
  const custom = form.querySelector(".date-custom");
  const presetGroup = form.querySelector(".date-presets");
  const chips = Array.from(form.querySelectorAll(".date-chip"));
  if (!(fromInput instanceof HTMLInputElement) || !(toInput instanceof HTMLInputElement)) return;
  if (!(custom instanceof HTMLElement) || !(presetGroup instanceof HTMLElement)) return;

  presetGroup.addEventListener("click", (event) => {
    const chip = event.target instanceof Element ? event.target.closest(".date-chip") : null;
    if (!(chip instanceof HTMLButtonElement)) return;
    const preset = chip.dataset.preset || "all";
    setActivePreset(chips, preset);

    if (preset === "custom") {
      custom.hidden = false;
      fromInput.focus();
      return;
    }

    if (preset === "all") {
      fromInput.value = "";
      toInput.value = "";
      custom.hidden = true;
      triggerChange(fromInput);
      return;
    }

    const range = rangeForDatePreset(preset, todayIso(), fromInput.min, toInput.max);
    fromInput.value = range.from;
    toInput.value = range.to;
    custom.hidden = true;
    triggerChange(fromInput);
  });

  fromInput.addEventListener("change", () => syncDatePresetState(form));
  toInput.addEventListener("change", () => syncDatePresetState(form));
  syncDatePresetState(form);
}

function closeSort(control) {
  const button = control.querySelector(".toolbar-sort");
  const menu = control.querySelector(".sort-menu");
  if (button instanceof HTMLButtonElement) button.setAttribute("aria-expanded", "false");
  if (menu instanceof HTMLElement) menu.hidden = true;
}

function initSortControl(control) {
  const input = control.querySelector(".toolbar-sort-value");
  const button = control.querySelector(".toolbar-sort");
  const menu = control.querySelector(".sort-menu");
  const label = control.querySelector("[data-sort-label]");
  if (!(input instanceof HTMLInputElement)) return;
  if (!(button instanceof HTMLButtonElement) || !(menu instanceof HTMLElement)) return;
  if (!(label instanceof HTMLElement)) return;

  let typeahead = "";
  let typeaheadAt = 0;

  const options = () =>
    /** @type {HTMLElement[]} */ (Array.from(menu.querySelectorAll(".sort-item")));

  function focusOption(index) {
    const target = options()[index];
    if (target instanceof HTMLElement) target.focus({ preventScroll: true });
  }

  function selectedIndex() {
    const found = options().findIndex((item) => item.getAttribute("aria-selected") === "true");
    return found < 0 ? 0 : found;
  }

  function openMenu() {
    document.querySelectorAll("[data-sort-control]").forEach((other) => {
      if (other !== control && other instanceof HTMLElement) closeSort(other);
    });
    menu.hidden = false;
    button.setAttribute("aria-expanded", "true");
    focusOption(selectedIndex());
  }

  button.addEventListener("click", (event) => {
    event.stopPropagation();
    if (menu.hidden) openMenu();
    else closeSort(control);
  });

  control.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      if (menu.hidden) return;
      // Without this the mobile filter overlay would close along with the menu.
      event.stopPropagation();
      event.preventDefault();
      closeSort(control);
      button.focus();
      return;
    }

    if (event.target === button) {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      event.preventDefault();
      if (menu.hidden) openMenu();
      else focusOption(selectedIndex());
      return;
    }

    if (menu.hidden || !menu.contains(/** @type {Node} */ (event.target))) return;

    const items = options();
    const current = items.indexOf(/** @type {HTMLElement} */ (document.activeElement));

    const next = nextSortIndex(event.key, current, items.length);
    if (next !== null) {
      event.preventDefault();
      focusOption(next);
      return;
    }

    // Space and Enter stay with the native button activation below.
    if (event.key.length !== 1 || event.key === " ") return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;

    const now = Date.now();
    typeahead = now - typeaheadAt > TYPEAHEAD_RESET_MS ? event.key : typeahead + event.key;
    typeaheadAt = now;
    const labels = items.map((item) => item.dataset.label || item.textContent?.trim() || "");
    const match = typeaheadIndex(labels, typeahead, current);
    if (match !== null) {
      event.preventDefault();
      focusOption(match);
    }
  });

  // Keep focus on the option the keyboard put it on: letting mousedown move
  // focus would fire focusout, hide the menu, and swallow the click.
  menu.addEventListener("mousedown", (event) => event.preventDefault());

  control.addEventListener("focusout", (event) => {
    if (menu.hidden) return;
    const next = event.relatedTarget;
    if (next instanceof Node && control.contains(next)) return;
    closeSort(control);
  });

  menu.addEventListener("click", (event) => {
    const item = event.target instanceof Element ? event.target.closest(".sort-item") : null;
    if (!(item instanceof HTMLButtonElement)) return;
    const value = item.dataset.value || "";
    const text = item.dataset.label || item.textContent?.trim() || value;
    input.value = value;
    const prefix = document.createElement("b");
    prefix.textContent = "Sort:";
    label.replaceChildren(prefix, ` ${text}`);
    menu.querySelectorAll(".sort-item").forEach((option) => {
      const active = option === item;
      option.classList.toggle("is-active", active);
      option.setAttribute("aria-selected", active ? "true" : "false");
    });
    closeSort(control);
    button.focus();
    triggerChange(input);
  });
}

function initListFilters(root = document) {
  root.querySelectorAll("[data-list-filters]").forEach((form) => {
    if (!(form instanceof HTMLFormElement) || form.dataset.enhancedFilters === "true") return;
    form.dataset.enhancedFilters = "true";
    initDatePresets(form);
    form.querySelectorAll("[data-sort-control]").forEach((control) => {
      if (control instanceof HTMLElement) initSortControl(control);
    });
  });
}

document.addEventListener("click", (event) => {
  if (event.target instanceof Element && event.target.closest("[data-sort-control]")) return;
  document.querySelectorAll("[data-sort-control]").forEach((control) => {
    if (control instanceof HTMLElement) closeSort(control);
  });
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  document.querySelectorAll("[data-sort-control]").forEach((control) => {
    if (control instanceof HTMLElement) closeSort(control);
  });
});

document.addEventListener("DOMContentLoaded", () => initListFilters());
document.addEventListener("htmx:afterSwap", (event) => {
  if (event.target instanceof Element) initListFilters(event.target);
});
