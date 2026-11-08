(function () {
  'use strict';

  function escapeHtml(value) {
    return String(value).replace(/[&<>"']/g, function (char) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char];
    });
  }

  function headingSlug(text) {
    return text.trim().toLowerCase().replace(/[^\p{L}\p{N}_\s-]/gu, '').replace(/\s+/g, '-');
  }

  function prepareDocument(fragment, root) {
    var headings = new Map();
    var usedIds = new Set();
    fragment.querySelectorAll('h1,h2,h3,h4,h5,h6').forEach(function (heading) {
      var slug = headingSlug(heading.textContent) || 'section';
      var id = 'md:' + slug;
      var suffix = 1;
      while (usedIds.has(id)) id = 'md:' + slug + '-' + suffix++;
      usedIds.add(id);
      heading.id = id;
      headings.set(id.slice(3), heading);
    });

    function findHeading(fragment) {
      return headings.get(headingSlug(fragment));
    }

    fragment.querySelectorAll('input').forEach(function (input) {
      if (input.type !== 'checkbox') input.remove();
      else input.disabled = true;
    });

    fragment.querySelectorAll('a[href]').forEach(function (link) {
      var href = link.getAttribute('href');
      try {
        if (href.charAt(0) === '#') {
          var heading = findHeading(decodeURIComponent(href.slice(1)));
          if (heading) link.setAttribute('href', '#' + encodeURIComponent(heading.id.slice(3)));
          else link.removeAttribute('href');
          return;
        }
        var url = new URL(href, window.location.href);
        if (!['http:', 'https:', 'mailto:', 'tel:'].includes(url.protocol)) {
          link.removeAttribute('href');
          return;
        }
        if (url.origin !== window.location.origin) {
          link.target = '_blank';
          link.rel = 'noopener noreferrer';
        } else if (url.pathname.startsWith(root.dataset.filesBase)) {
          var at = new URL(window.location.href).searchParams.get('at');
          if (at && !url.searchParams.has('at')) url.searchParams.set('at', at);
        }
        link.setAttribute('href', url.href);
      } catch (_) {
        link.removeAttribute('href');
      }
    });

    fragment.querySelectorAll('table').forEach(function (table) {
      var scroll = document.createElement('div');
      scroll.setAttribute('data-markdown-table', '');
      scroll.tabIndex = 0;
      table.replaceWith(scroll);
      scroll.append(table);
    });
    return findHeading;
  }

  function render(source, root) {
    var frontmatter = /^\uFEFF?---[ \t]*\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/.exec(source);
    var body = frontmatter ? source.slice(frontmatter[0].length) : source;
    var renderer = new window.marked.Renderer();
    renderer.image = function (token) {
      var label = root.dataset.imageLabel + (token.text ? ': ' + token.text : '');
      return '<a href="' + escapeHtml(token.href) + '">' + escapeHtml(label) + '</a>';
    };
    var html = window.marked.parse(body, { gfm: true, async: false, renderer: renderer });
    var fragment = window.DOMPurify.sanitize(html, {
      ALLOWED_TAGS: [
        'p', 'br', 'hr', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote',
        'ul', 'ol', 'li', 'pre', 'code', 'em', 'strong', 'del', 's', 'a',
        'table', 'thead', 'tbody', 'tfoot', 'tr', 'th', 'td', 'input',
        'sup', 'sub', 'kbd', 'details', 'summary', 'dl', 'dt', 'dd'
      ],
      ALLOWED_ATTR: ['href', 'title', 'align', 'start', 'reversed', 'type', 'checked', 'disabled', 'open', 'colspan', 'rowspan'],
      ALLOW_DATA_ATTR: false,
      ALLOW_ARIA_ATTR: false,
      FORBID_ATTR: ['id', 'name', 'style', 'class'],
      RETURN_DOM_FRAGMENT: true
    });
    var findHeading = prepareDocument(fragment, root);
    if (frontmatter) {
      var properties = document.createElement('details');
      properties.setAttribute('data-markdown-properties', '');
      var summary = document.createElement('summary');
      summary.textContent = root.dataset.propertiesLabel;
      var values = document.createElement('pre');
      values.textContent = frontmatter[1];
      properties.append(summary, values);
      fragment.prepend(properties);
    }
    return { fragment: fragment, findHeading: findHeading };
  }

  function initViewer(root) {
    if (root.dataset.markdownReady) return;
    root.dataset.markdownReady = 'true';
    var source = root.querySelector('[data-markdown-source]');
    var preview = root.querySelector('[data-markdown-preview]');
    var sourcePanel = root.querySelector('[data-markdown-source-panel]');
    var tabs = root.querySelector('[data-markdown-tabs]');
    var error = root.querySelector('[data-markdown-error]');
    var buttons = Array.from(tabs.querySelectorAll('[data-markdown-mode]'));

    function select(mode, focus) {
      preview.hidden = mode !== 'preview';
      sourcePanel.hidden = mode !== 'source';
      buttons.forEach(function (button) {
        var selected = button.dataset.markdownMode === mode;
        button.setAttribute('aria-selected', String(selected));
        button.tabIndex = selected ? 0 : -1;
        if (selected && focus) button.focus();
      });
    }

    try {
      if (!window.marked || !window.DOMPurify) throw new Error('Markdown dependencies unavailable');
      var rendered = render(source.textContent, root);
      preview.replaceChildren(rendered.fragment);
      function scrollToFragment() {
        if (!root.isConnected || preview.hidden) return;
        try {
          var heading = rendered.findHeading(decodeURIComponent(window.location.hash.slice(1)));
          if (heading) heading.scrollIntoView({ block: 'start' });
        } catch (_) { /* A malformed URL fragment must not hide readable content. */ }
      }
      preview.addEventListener('click', function (event) {
        if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
        var link = event.target.closest && event.target.closest('a[href]');
        if (!link || !preview.contains(link)) return;
        var url = new URL(link.href, window.location.href);
        var current = new URL(window.location.href);
        if (url.origin !== current.origin || url.pathname !== current.pathname || url.search !== current.search || !url.hash) return;
        var heading;
        try { heading = rendered.findHeading(decodeURIComponent(url.hash.slice(1))); }
        catch (_) { return; }
        if (!heading) return;
        event.preventDefault();
        if (url.hash !== current.hash) window.history.pushState(window.history.state, '', url.hash);
        scrollToFragment();
      });
      buttons.forEach(function (button, index) {
        button.addEventListener('click', function () { select(button.dataset.markdownMode, false); });
        button.addEventListener('keydown', function (event) {
          var next;
          if (event.key === 'ArrowRight') next = (index + 1) % buttons.length;
          else if (event.key === 'ArrowLeft') next = (index + buttons.length - 1) % buttons.length;
          else if (event.key === 'Home') next = 0;
          else if (event.key === 'End') next = buttons.length - 1;
          else return;
          event.preventDefault();
          select(buttons[next].dataset.markdownMode, true);
        });
      });
      tabs.hidden = false;
      select('preview', false);
      scrollToFragment();
      // Native initial fragment navigation can run after deferred scripts.
      function afterLoad() { window.requestAnimationFrame(scrollToFragment); }
      if (document.readyState === 'complete') afterLoad();
      else window.addEventListener('load', afterLoad, { once: true });
      window.addEventListener('hashchange', scrollToFragment);
    } catch (_) {
      preview.replaceChildren();
      preview.hidden = true;
      sourcePanel.hidden = false;
      tabs.hidden = true;
      error.textContent = root.dataset.previewErrorText;
      error.hidden = false;
    }
  }

  function init() {
    document.querySelectorAll('[data-markdown-viewer]').forEach(initViewer);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init, { once: true });
  else init();
})();
