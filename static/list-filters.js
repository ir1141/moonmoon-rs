import { datePresetForRange, rangeForDatePreset } from "./lib/list-filters.js";

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

  button.addEventListener("click", (event) => {
    event.stopPropagation();
    const open = menu.hidden;
    document.querySelectorAll("[data-sort-control]").forEach((other) => {
      if (other !== control && other instanceof HTMLElement) closeSort(other);
    });
    menu.hidden = !open;
    button.setAttribute("aria-expanded", String(open));
  });

  menu.addEventListener("click", (event) => {
    const item = event.target instanceof Element ? event.target.closest(".sort-item") : null;
    if (!(item instanceof HTMLButtonElement)) return;
    const value = item.dataset.value || "";
    const text = item.dataset.label || item.textContent?.trim() || value;
    input.value = value;
    label.innerHTML = `<b>Sort:</b> ${text}`;
    menu.querySelectorAll(".sort-item").forEach((option) => {
      const active = option === item;
      option.classList.toggle("is-active", active);
      option.setAttribute("aria-selected", active ? "true" : "false");
    });
    closeSort(control);
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
