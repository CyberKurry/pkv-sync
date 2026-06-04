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

  function createConfirmDialog() {
    var dialog = document.createElement("dialog");
    dialog.className = "confirm-dialog";
    dialog.innerHTML =
      '<form method="dialog" class="confirm-dialog-card">' +
      '<div class="confirm-dialog-icon" aria-hidden="true">' +
      '<svg class="admin-icon"><use href="/admin/static/lucide-icons.svg#ban"></use></svg>' +
      "</div>" +
      '<div class="confirm-dialog-copy">' +
      '<h2 data-confirm-dialog-title></h2>' +
      '<p data-confirm-dialog-body></p>' +
      "</div>" +
      '<div class="confirm-dialog-actions">' +
      '<button class="secondary" value="cancel" data-confirm-dialog-cancel></button>' +
      '<button class="danger" value="confirm" data-confirm-dialog-confirm></button>' +
      "</div>" +
      "</form>";
    document.body.appendChild(dialog);
    return dialog;
  }

  function configureDialog(dialog, title, body, confirmLabel, cancelLabel, mode) {
    var confirm = dialog.querySelector("[data-confirm-dialog-confirm]");
    var cancel = dialog.querySelector("[data-confirm-dialog-cancel]");
    dialog.querySelector("[data-confirm-dialog-title]").textContent = title;
    dialog.querySelector("[data-confirm-dialog-body]").textContent = body || "";
    confirm.textContent = confirmLabel;
    cancel.textContent = cancelLabel;
    confirm.className = mode === "notice" ? "secondary" : "danger";
    dialog.dataset.mode = mode || "confirm";
  }

  function confirmSubmit(button) {
    var form = button.closest("form");
    if (!form) return;
    if (button.dataset.confirmed === "true") return;

    var title = button.dataset.confirmTitle || "Confirm action?";
    var body = button.dataset.confirmBody || "";
    var confirmLabel = button.dataset.confirmConfirm || "Continue";
    var cancelLabel = button.dataset.confirmCancel || "Cancel";
    var dialog = document.querySelector(".confirm-dialog") || createConfirmDialog();

    if (typeof dialog.showModal !== "function") {
      if (window.confirm(body ? title + "\n\n" + body : title)) {
        button.dataset.confirmed = "true";
        if (typeof form.requestSubmit === "function") form.requestSubmit(button);
        else form.submit();
      }
      return;
    }

    configureDialog(dialog, title, body, confirmLabel, cancelLabel, "confirm");
    dialog.returnValue = "";

    var onClose = function () {
      dialog.removeEventListener("close", onClose);
      if (dialog.returnValue !== "confirm") return;
      button.dataset.confirmed = "true";
      if (typeof form.requestSubmit === "function") form.requestSubmit(button);
      else form.submit();
    };
    dialog.addEventListener("close", onClose);
    dialog.showModal();
  }

  function showErrorDialog(message) {
    var dialog = document.querySelector(".confirm-dialog") || createConfirmDialog();
    if (typeof dialog.showModal !== "function") return;
    configureDialog(dialog, message, "", "OK", "", "notice");
    dialog.returnValue = "";
    dialog.showModal();
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

    var confirmButtons = document.querySelectorAll("[data-confirm-title]");
    confirmButtons.forEach(function (button) {
      button.addEventListener("click", function (event) {
        if (button.dataset.confirmed === "true") {
          delete button.dataset.confirmed;
          return;
        }
        event.preventDefault();
        confirmSubmit(button);
      });
    });

    var errorNotice = document.querySelector("[data-error-dialog-title]");
    if (errorNotice) {
      showErrorDialog(errorNotice.dataset.errorDialogTitle || errorNotice.textContent);
    }
  });
})();
