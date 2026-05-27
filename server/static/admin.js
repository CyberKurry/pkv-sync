(function () {
  var storageKey = "pkv-admin-theme";
  var modes = ["auto", "light", "dark"];
  var icons = {
    auto: "monitor",
    light: "sun",
    dark: "moon"
  };

  function normalizeMode(value) {
    return value === "light" || value === "dark" ? value : "auto";
  }

  function applyTheme(mode) {
    if (mode === "light" || mode === "dark") {
      localStorage.setItem(storageKey, mode);
      document.documentElement.dataset.theme = mode;
      return;
    }
    localStorage.removeItem(storageKey);
    delete document.documentElement.dataset.theme;
  }

  function labelFor(button, mode) {
    if (mode === "light") return button.dataset.labelLight || "Light";
    if (mode === "dark") return button.dataset.labelDark || "Dark";
    return button.dataset.labelAuto || "Auto";
  }

  function renderThemeButton(button, mode) {
    var label = labelFor(button, mode);
    var themeLabel = button.dataset.labelTheme || "Theme";
    var iconUse = button.querySelector("[data-theme-icon-use]");
    var text = button.querySelector("[data-theme-label]");
    button.dataset.themeMode = mode;
    button.setAttribute("aria-label", themeLabel + ": " + label);
    button.setAttribute("title", themeLabel + ": " + label);
    if (iconUse) {
      iconUse.setAttribute(
        "href",
        "/admin/static/lucide-icons.svg#" + icons[mode]
      );
    }
    if (text) text.textContent = label;
  }

  function renderThemeButtons(mode) {
    var buttons = document.querySelectorAll("[data-theme-toggle]");
    buttons.forEach(function (button) {
      renderThemeButton(button, mode);
    });
  }

  function nextMode(mode) {
    var index = modes.indexOf(mode);
    return modes[(index + 1) % modes.length];
  }

  function submitClosestForm(control) {
    var form = control.closest("form");
    if (!form) return;
    if (typeof form.requestSubmit === "function") {
      form.requestSubmit();
      return;
    }
    form.submit();
  }

  applyTheme(normalizeMode(localStorage.getItem(storageKey)));

  document.addEventListener("DOMContentLoaded", function () {
    var languageSelects = document.querySelectorAll("[data-language-select]");
    languageSelects.forEach(function (languageSelect) {
      languageSelect.addEventListener("change", function () {
        submitClosestForm(languageSelect);
      });
    });

    var autoSubmitFilters = document.querySelectorAll("[data-auto-submit-filter]");
    autoSubmitFilters.forEach(function (filter) {
      filter.addEventListener("change", function () {
        submitClosestForm(filter);
      });
    });

    var mode = normalizeMode(localStorage.getItem(storageKey));
    renderThemeButtons(mode);
    var buttons = document.querySelectorAll("[data-theme-toggle]");
    buttons.forEach(function (button) {
      button.addEventListener("click", function () {
        mode = nextMode(mode);
        applyTheme(mode);
        renderThemeButtons(mode);
      });
    });
  });
})();
