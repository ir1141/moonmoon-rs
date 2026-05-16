function initMobileSearch() {
  const headerRight = /** @type {HTMLElement | null} */ (
    document.querySelector(".header-right")
  );
  const toggle = /** @type {HTMLButtonElement | null} */ (
    document.querySelector(".search-toggle")
  );
  const input = /** @type {HTMLInputElement | null} */ (
    document.querySelector(".search-bar input")
  );
  if (!headerRight || !toggle || !input) return;

  function setOpen(open) {
    headerRight.classList.toggle("search-open", open);
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
    if (open) {
      window.requestAnimationFrame(() => input.focus());
    }
  }

  toggle.addEventListener("click", () => {
    setOpen(!headerRight.classList.contains("search-open"));
  });

  document.addEventListener("keydown", (event) => {
    if (
      event.key === "Escape" &&
      headerRight.classList.contains("search-open")
    ) {
      setOpen(false);
      toggle.focus();
    }
  });
}

initMobileSearch();
