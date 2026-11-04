// mhtml adaptive presentation: progressive table controls, code block tools
// and the side outline's current-section marker. The page reads fine without
// it; everything here only adds controls to markup the renderer produced.
(function () {
  'use strict';
  var lang = document.documentElement.lang || 'en';
  var cjk = /^(zh|ja|ko)/i.test(lang);
  var T = cjk
    ? { filter: '篩選表格內容…', rows: function (n, t) { return n === t ? '共 ' + t + ' 列' : '顯示 ' + n + ' / ' + t + ' 列'; },
        scroll: '← 可左右捲動 →', sort: '排序', resize: '拖曳調整欄寬（雙擊還原）',
        reset: '還原順序與欄寬', resetDone: '已還原原始順序與欄寬', unsorted: '已還原原始順序',
        sorted: function (name, up) { return '已依 ' + name + (up ? ' 遞增' : ' 遞減') + '排序'; },
        copy: '複製', copied: '已複製 ✓', table: '表格' }
    : { filter: 'Filter rows…', rows: function (n, t) { return n === t ? t + ' rows' : n + ' of ' + t + ' rows'; },
        scroll: '← scroll sideways →', sort: 'Sort', resize: 'Drag to resize (double-click to reset)',
        reset: 'Reset order and widths', resetDone: 'Original order and widths restored', unsorted: 'Original order restored',
        sorted: function (name, up) { return 'Sorted by ' + name + (up ? ' ascending' : ' descending'); },
        copy: 'Copy', copied: 'Copied ✓', table: 'Table' };
  var collator = new Intl.Collator(lang, { numeric: true, sensitivity: 'base' });

  function text(el) { return (el.textContent || '').replace(/\s+/g, ' ').trim(); }

  function nearestHeading(el) {
    for (var n = el.previousElementSibling; n; n = n.previousElementSibling) {
      if (/^H[1-6]$/.test(n.tagName)) return text(n).replace(/#$/, '');
    }
    return '';
  }

  // ---- tables -------------------------------------------------------------

  function numberOf(s) {
    var m = s.replace(/[,\s]/g, '').match(/^[~約≈]?(-?\d+(?:\.\d+)?)/);
    return m ? parseFloat(m[1]) : NaN;
  }

  function setupTable(wrap) {
    var table = wrap.querySelector(':scope > table');
    if (!table || !table.tHead || !table.tBodies[0]) return;
    var head = table.tHead.rows[0];
    var body = table.tBodies[0];
    var rows = Array.prototype.slice.call(body.rows);
    rows.forEach(function (r, i) { r.mxIndex = i; });
    var headers = Array.prototype.map.call(head.cells, function (th) { return text(th); });
    var label = nearestHeading(wrap) || headers.join(' / ');
    var tools = null;

    function toolbar() {
      if (!tools) {
        tools = document.createElement('div');
        tools.className = 'table-tools';
        wrap.parentNode.insertBefore(tools, wrap);
      }
      return tools;
    }

    var cells = Array.prototype.slice.call(head.cells);
    var live = null;
    function announce(msg) {
      if (!live) {
        live = document.createElement('span');
        live.className = 'visually-hidden';
        live.setAttribute('aria-live', 'polite');
        toolbar().appendChild(live);
      }
      live.textContent = msg;
    }
    function resetWidths() {
      wrap.classList.remove('is-resized');
      table.style.width = '';
      cells.forEach(function (c) { c.style.width = ''; });
      update();
    }

    if (wrap.classList.contains('stackable')) {
      rows.forEach(function (r) {
        Array.prototype.forEach.call(r.cells, function (td, i) {
          if (headers[i]) td.setAttribute('data-label', headers[i]);
        });
      });
      wrap.classList.add('has-labels');
    }

    if (wrap.classList.contains('sortable')) {
      var state = { col: -1, dir: 0 };
      Array.prototype.forEach.call(head.cells, function (th, col) {
        if (th.querySelector('a, button, input')) return;
        var btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'th-sort';
        btn.title = T.sort;
        while (th.firstChild) btn.appendChild(th.firstChild);
        th.appendChild(btn);
        btn.addEventListener('click', function () {
          state.dir = state.col === col ? (state.dir + 1) % 3 : 1;
          state.col = col;
          sortBy(col, state.dir);
          Array.prototype.forEach.call(head.cells, function (h, i) {
            if (i === col && state.dir) h.setAttribute('aria-sort', state.dir === 1 ? 'ascending' : 'descending');
            else h.removeAttribute('aria-sort');
          });
          announce(state.dir === 1 ? T.sorted(headers[col], true)
            : state.dir === 2 ? T.sorted(headers[col], false) : T.unsorted);
        });
      });

      var sortBy = function (col, dir) {
        var keys = rows.map(function (r) { return r.cells[col] ? text(r.cells[col]) : ''; });
        var filled = keys.filter(Boolean);
        var numeric = filled.length > 0 &&
          filled.filter(function (k) { return !isNaN(numberOf(k)); }).length >= filled.length * 0.8;
        var order = rows.map(function (r, i) { return i; });
        if (dir) {
          order.sort(function (a, b) {
            var x = keys[a], y = keys[b];
            if (!x || !y) return x ? -1 : y ? 1 : a - b; // blanks last
            var c = numeric ? numberOf(x) - numberOf(y) : collator.compare(x, y);
            if (isNaN(c)) c = collator.compare(x, y);
            return (c || a - b) * (dir === 1 ? 1 : -1);
          });
        }
        var frag = document.createDocumentFragment();
        order.forEach(function (i) { frag.appendChild(rows[i]); });
        body.appendChild(frag);
      };
    }

    if (wrap.classList.contains('filterable')) {
      var bar = toolbar();
      var input = document.createElement('input');
      input.type = 'search';
      input.placeholder = T.filter;
      input.setAttribute('aria-label', T.filter + (label ? ' — ' + label : ''));
      var count = document.createElement('span');
      count.className = 'count';
      count.setAttribute('aria-live', 'polite');
      count.textContent = T.rows(rows.length, rows.length);
      bar.appendChild(input);
      bar.appendChild(count);
      var texts = rows.map(function (r) { return text(r).toLowerCase(); });
      input.addEventListener('input', function () {
        var q = input.value.trim().toLowerCase();
        var shown = 0;
        rows.forEach(function (r, i) {
          var hit = !q || texts[i].indexOf(q) !== -1;
          r.hidden = !hit;
          if (hit) shown++;
        });
        count.textContent = T.rows(shown, rows.length);
      });
    }

    if (wrap.classList.contains('resizable')) {
      cells.slice(0, -1).forEach(function (th, i) {
        var grip = document.createElement('span');
        grip.className = 'col-resize';
        grip.tabIndex = 0;
        grip.setAttribute('role', 'separator');
        grip.setAttribute('aria-orientation', 'vertical');
        grip.setAttribute('aria-label', T.resize + ' — ' + (headers[i] || i + 1));
        grip.title = T.resize;
        th.appendChild(grip);
        grip.addEventListener('click', function (e) { e.stopPropagation(); });
        grip.addEventListener('pointerdown', function (e) {
          e.preventDefault();
          freeze();
          var startX = e.clientX, startW = th.offsetWidth, startT = table.offsetWidth;
          grip.setPointerCapture(e.pointerId);
          grip.classList.add('is-active');
          function move(ev) { resize(th, startW, startT, ev.clientX - startX); }
          function up() {
            grip.classList.remove('is-active');
            grip.removeEventListener('pointermove', move);
            grip.removeEventListener('pointerup', up);
            grip.removeEventListener('pointercancel', up);
          }
          grip.addEventListener('pointermove', move);
          grip.addEventListener('pointerup', up);
          grip.addEventListener('pointercancel', up);
        });
        grip.addEventListener('keydown', function (e) {
          var step = e.key === 'ArrowRight' ? 24 : e.key === 'ArrowLeft' ? -24 : 0;
          if (!step) return;
          e.preventDefault();
          freeze();
          resize(th, th.offsetWidth, table.offsetWidth, step);
        });
        grip.addEventListener('dblclick', function (e) {
          e.preventDefault();
          resetWidths();
        });
      });
      var freeze = function () {
        if (wrap.classList.contains('is-resized')) return;
        var widths = cells.map(function (c) { return c.getBoundingClientRect().width; });
        table.style.width = table.getBoundingClientRect().width + 'px';
        cells.forEach(function (c, i) { c.style.width = widths[i] + 'px'; });
        wrap.classList.add('is-resized');
      };
      var resize = function (th, startW, startT, dx) {
        var w = Math.max(48, startW + dx);
        th.style.width = w + 'px';
        table.style.width = startT + (w - startW) + 'px';
        update();
      };
    }

    // One control to bring back the original order and column widths.
    if (wrap.classList.contains('sortable') || wrap.classList.contains('resizable')) {
      var reset = document.createElement('button');
      reset.type = 'button';
      reset.className = 'table-reset';
      reset.textContent = T.reset;
      reset.title = T.reset;
      toolbar().appendChild(reset);
      reset.addEventListener('click', function () {
        if (wrap.classList.contains('sortable')) {
          state.col = -1;
          state.dir = 0;
          sortBy(0, 0);
          Array.prototype.forEach.call(head.cells, function (h) { h.removeAttribute('aria-sort'); });
        }
        if (wrap.classList.contains('resizable')) resetWidths();
        announce(T.resetDone);
      });
    }

    // Scroll affordances: keyboard focus, a name, and a hint when it overflows.
    var hint = null;
    function update() {
      var scrollable = wrap.scrollWidth > wrap.clientWidth + 1 || wrap.scrollHeight > wrap.clientHeight + 1;
      if (scrollable) {
        wrap.tabIndex = 0;
        wrap.setAttribute('role', 'region');
        wrap.setAttribute('aria-label', label || T.table);
      } else {
        wrap.removeAttribute('tabindex');
        wrap.removeAttribute('role');
        wrap.removeAttribute('aria-label');
      }
      var sideways = wrap.scrollWidth > wrap.clientWidth + 1;
      if (sideways && !hint) {
        hint = document.createElement('span');
        hint.className = 'hint';
        hint.textContent = T.scroll;
        toolbar().appendChild(hint);
      }
      if (hint) hint.hidden = !sideways;
    }
    update();
    if (window.ResizeObserver) new ResizeObserver(update).observe(wrap);
  }

  // ---- code blocks ----------------------------------------------------------

  var SVG_NS = 'http://www.w3.org/2000/svg';

  // Stroke icons built without innerHTML: two rounded rectangles for "copy",
  // a check mark for the copied state.
  function svgIcon(kind) {
    var svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', 'currentColor');
    svg.setAttribute('stroke-width', '2');
    svg.setAttribute('stroke-linecap', 'round');
    svg.setAttribute('stroke-linejoin', 'round');
    svg.setAttribute('aria-hidden', 'true');
    svg.classList.add('code-copy-glyph');
    var parts = kind === 'check'
      ? [['polyline', { points: '20 6 9 17 4 12' }]]
      : [['rect', { x: 9, y: 9, width: 13, height: 13, rx: 2 }],
         ['path', { d: 'M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1' }]];
    parts.forEach(function (p) {
      var el = document.createElementNS(SVG_NS, p[0]);
      Object.keys(p[1]).forEach(function (k) { el.setAttribute(k, p[1][k]); });
      svg.appendChild(el);
    });
    return svg;
  }

  function setupCode(pre) {
    if (pre.closest('.mermaid-diagram, .code-block')) return;
    var code = pre.querySelector('code');
    var wrap = document.createElement('div');
    wrap.className = 'code-block';
    pre.parentNode.insertBefore(wrap, pre);
    wrap.appendChild(pre);
    var m = code && /(?:^|\s)language-(\S+)/.exec(code.className);
    if (m) {
      var tag = document.createElement('span');
      tag.className = 'code-lang';
      tag.textContent = m[1];
      wrap.appendChild(tag);
    }
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'code-copy';
    btn.title = T.copy;
    btn.setAttribute('aria-label', T.copy);
    btn.appendChild(svgIcon('copy'));
    wrap.appendChild(btn);
    var restore = null;
    function glyph(kind) {
      while (btn.firstChild) btn.removeChild(btn.firstChild);
      btn.appendChild(svgIcon(kind));
    }
    btn.addEventListener('click', function () {
      var src = (code || pre).innerText.replace(/\n$/, '');
      var done = function () {
        glyph('check');
        btn.classList.add('is-copied');
        btn.title = T.copied;
        btn.setAttribute('aria-label', T.copied);
        if (restore) clearTimeout(restore);
        restore = setTimeout(function () {
          glyph('copy');
          btn.classList.remove('is-copied');
          btn.title = T.copy;
          btn.setAttribute('aria-label', T.copy);
        }, 1600);
      };
      if (navigator.clipboard && window.isSecureContext) {
        navigator.clipboard.writeText(src).then(done, fallback);
      } else {
        fallback();
      }
      function fallback() {
        var range = document.createRange();
        range.selectNodeContents(code || pre);
        var sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
        try { if (document.execCommand('copy')) done(); } catch (e) { /* leave it selected */ }
      }
    });
  }

  // ---- side outline -------------------------------------------------------

  function setupOutline(nav) {
    var aside = nav.closest('.mhtml-aside');
    var links = Array.prototype.slice.call(nav.querySelectorAll('a[href^="#"]'));
    var targets = links.map(function (a) {
      return document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1)));
    });
    var folding = links.length > 30;
    if (folding) nav.classList.add('is-folding');
    var current = null, queued = false;
    function sync() {
      queued = false;
      var line = Math.min(120, window.innerHeight * 0.25);
      var pick = -1;
      targets.forEach(function (t, i) {
        if (t && t.getBoundingClientRect().top <= line) pick = i;
      });
      if (pick < 0) pick = 0;
      var link = links[pick];
      if (link === current) return;
      if (current) current.removeAttribute('aria-current');
      current = link;
      if (!link) return;
      link.setAttribute('aria-current', 'true');
      if (folding) {
        nav.querySelectorAll('li.is-open').forEach(function (li) { li.classList.remove('is-open'); });
        for (var li = link.closest('li'); li && nav.contains(li); li = li.parentElement.closest('li')) {
          li.classList.add('is-open');
        }
      }
      if (aside && aside.scrollHeight > aside.clientHeight) {
        var a = aside.getBoundingClientRect(), l = link.getBoundingClientRect();
        if (l.top < a.top + 40 || l.bottom > a.bottom - 40) {
          aside.scrollTop += l.top - a.top - a.height / 3;
        }
      }
    }
    function queue() {
      if (!queued) { queued = true; requestAnimationFrame(sync); }
    }
    window.addEventListener('scroll', queue, { passive: true });
    window.addEventListener('resize', queue);
    sync();
  }

  document.querySelectorAll('.markdown-body .table-wrap').forEach(setupTable);
  document.querySelectorAll('.markdown-body pre').forEach(setupCode);
  document.querySelectorAll('nav.toc-side').forEach(setupOutline);
})();
