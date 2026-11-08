// Kaeri's novel paginator: one script for WKWebView, Android WebView, WebView2 and WebKitGTK.
//
// The host injects it into every book document (an isolated world where there is one), the
// engine serves the document with the reader's settings in <meta name="kaeri-setup">, and the
// script lays the document out in pages, keeps the reader's place as a structural position,
// and talks to the host in JSON messages. It never adds nodes to <body>, so a position means
// the same thing in every engine.
//
// Where to start comes from the URL's fragment:
//   #kaeri:end              the last page (going back from the next document)
//   #kaeri:pos=3/0.4.1/120  a position, as NovelPos text (the spine part is ignored)
//   #some-id                the element with that id (a contents entry, a footnote)
//   nothing                 the first page
//
// Messages to the host, each a JSON string:
//   ready     {spine, page, pages, pos, vertical, failed}  first layout done; forward as NovelAt
//   page      {spine, page, pages, pos, how, last}         a turn; forward as NovelAt
//   layout    {spine, page, pages}                         laid out again (settings, resize);
//                                                          the place is unchanged, nothing to forward
//   end       {spine}                                      a turn past the last page
//   start     {spine}                                      a turn before the first page
//   link      {href, internal, note}                       a link was tapped; nothing was followed
//   tap       {x, y}                                       a tap off any link, as fractions of the view
//   key       {key}                                        a turning key the page received
//   selection {text, start, end, rect}  and  unselected {}
//   failed    {spine, why}                                 the engine could not serve this document
//
// The host calls window.kaeriReader: next(how), prev(how), goTo(page, how), goToPos(pos, how),
// goToFragment(id, how), configure(style), state(). `how` is one of turn, swipe, key, jump,
// scroll, echoed back in `page`.
(function () {
  'use strict';
  if (window.top !== window) return; // Chromium runs document-start scripts in frames too
  if (window.kaeriReader) return;
  // Only a book's documents: a host's user script also meets about:blank and its own pages.
  if (!/^(kaeri-book:|https:\/\/appassets\.androidplatform\.net\/book\/)/.test(location.href)) return;

  var CSS = "__KAERI_CSS__";
  var SHY = /­/g;
  var XHTML = 'http://www.w3.org/1999/xhtml';
  var DEFAULTS = {
    spine: 0, last: false,
    look: { name: 'light', bg: '#F7F5F0', fg: '#1E2033', link: '#1E2033', ink: false },
    font: '"Literata", "Iowan Old Style", Charter, Georgia, serif',
    size: 19, line: 1.5, margin: 28, top: 40, bottom: 48,
    justify: true, hyphens: true, mode: 'paged', columns: 'auto', column: 720,
    measure: [65, 75], eink: false, legacy: false, fixed: false
  };

  var S = {};
  var state = {
    ready: false, axis: 'x', vertical: false, rtl: false, scrolled: false, page: 0, pages: 1,
    anchor: null, nodes: null, selection: null, settling: 0, resizing: 0, fixedScale: 1
  };
  var rng = document.createRange();

  function merge(into, from) {
    if (!from || typeof from !== 'object') return into;
    for (var k in from) {
      if (!Object.prototype.hasOwnProperty.call(from, k)) continue;
      if (from[k] && typeof from[k] === 'object' && !Array.isArray(from[k]) && into[k] && typeof into[k] === 'object') {
        into[k] = merge(merge({}, into[k]), from[k]);
      } else {
        into[k] = from[k];
      }
    }
    return into;
  }

  function setup() {
    var s = merge({}, DEFAULTS);
    var meta = document.querySelector('meta[name="kaeri-setup"]');
    if (meta) { try { s = merge(s, JSON.parse(meta.getAttribute('content'))); } catch (e) { /* defaults */ } }
    if (window.kaeriSetup) s = merge(s, typeof window.kaeriSetup === 'string' ? JSON.parse(window.kaeriSetup) : window.kaeriSetup);
    return s;
  }

  function post(msg) {
    var s = JSON.stringify(msg);
    try {
      if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.kaeri) {
        window.webkit.messageHandlers.kaeri.postMessage(s); return;
      }
      if (typeof window.kaeriHost === 'function') { window.kaeriHost(s); return; }
      if (window.kaeriHost && window.kaeriHost.postMessage) { window.kaeriHost.postMessage(s); return; }
      if (window.chrome && window.chrome.webview) { window.chrome.webview.postMessage(s); return; }
    } catch (e) { /* the host is gone */ }
  }

  // ---- Styles -------------------------------------------------------------------------

  function styleEl() {
    var el = document.getElementById('kaeri-reader-style');
    if (!el) {
      el = document.createElementNS(XHTML, 'style');
      el.id = 'kaeri-reader-style';
      el.textContent = CSS;
      var head = document.head || document.querySelector('head');
      // A head-less document gets the style on <html>, outside <body>.
      if (head) head.appendChild(el); else document.documentElement.insertBefore(el, document.body);
    }
    return el;
  }

  function lang() {
    var h = document.documentElement;
    return (h.getAttributeNS('http://www.w3.org/XML/1998/namespace', 'lang') || h.getAttribute('lang') || '').toLowerCase();
  }
  function cjk() { return /^(ja|zh|ko)/.test(lang()); }

  function detectVertical() {
    var b = document.body ? getComputedStyle(document.body) : null;
    var wm = b ? (b.writingMode || b.webkitWritingMode || '') : '';
    var root = getComputedStyle(document.documentElement).writingMode || '';
    var v = /^(vertical|tb)/.test(wm) || /^(vertical|tb)/.test(root);
    // The multicol container (<html>) must share the body's writing mode.
    if (v && !/vertical/.test(root)) {
      document.documentElement.style.setProperty('writing-mode', wm.indexOf('lr') >= 0 ? 'vertical-lr' : 'vertical-rl');
    }
    return v;
  }

  function fixedSize() {
    // The engine renames the book's viewport and serves one as wide as the window.
    var meta = document.querySelector('meta[name="kaeri-viewport"]');
    var c = meta ? meta.getAttribute('content') || '' : '';
    var w = /width\s*=\s*(\d+)/.exec(c), h = /height\s*=\s*(\d+)/.exec(c);
    if (w && h) return { w: +w[1], h: +h[1] };
    var svg = document.body && document.body.querySelector('svg');
    var vb = svg && svg.viewBox && svg.viewBox.baseVal;
    if (vb && vb.width && vb.height) return { w: vb.width, h: vb.height };
    return null;
  }

  // The line's length in characters, at least and at most, as px at the text size. A Latin
  // character of running text in a book face averages about 0.45 em; CJK characters are a
  // whole em, and their lines run 30 to 40 where Latin runs 65 to 75.
  function measure() {
    var m = S.measure || DEFAULTS.measure;
    var em = cjk() ? 1 : 0.45;
    var chars = cjk() ? [m[0] * 6 / 13, m[1] * 8 / 15] : m;
    return { min: chars[0] * em * S.size, max: chars[1] * em * S.size };
  }

  function columns(width) {
    if (state.vertical || state.scrolled) return 1;
    if (S.columns === 1 || S.columns === 2) return S.columns;
    if (S.eink) return 1;
    // Two columns only where each still holds the shortest line.
    return (width - 4 * S.margin) / 2 >= measure().min ? 2 : 1;
  }

  function apply() {
    var h = document.documentElement;
    styleEl();
    h.classList.add('kaeri');
    state.vertical = detectVertical();
    state.scrolled = S.mode === 'scrolled' && !state.vertical && !S.fixed;
    state.rtl = !state.vertical && getComputedStyle(h).direction === 'rtl';
    state.axis = state.vertical || state.scrolled ? 'y' : 'x';
    var W = vw();
    var cols = columns(W);
    var margin = S.margin;
    // The column is capped near 720 px and the longest line: wider windows get wider margins.
    var cap = Math.min(S.column, measure().max);
    if (!state.vertical && !state.scrolled) {
      var col = (W - 2 * margin * cols) / cols;
      if (col > cap) margin = (W - cap * cols) / (2 * cols);
    }
    var look = S.look || {};
    var set = function (k, v) { h.style.setProperty(k, v); };
    set('--k-bg', look.bg || DEFAULTS.look.bg);
    set('--k-fg', look.fg || DEFAULTS.look.fg);
    set('--k-link', look.link || look.fg || DEFAULTS.look.fg);
    if (S.font) set('--k-font', S.font);
    set('--k-size', S.size + 'px');
    set('--k-line', String(S.line));
    set('--k-margin', margin + 'px');
    set('--k-top', S.top + 'px');
    set('--k-bottom', S.bottom + 'px');
    set('--k-cols', String(cols));
    set('--k-column', cap + 'px');
    h.classList.toggle('k-book-font', !S.font);
    h.classList.toggle('k-justify', !!S.justify);
    h.classList.toggle('k-hyphens', !!S.hyphens);
    h.classList.toggle('k-ink', !!(look.ink || S.eink));
    h.classList.toggle('k-eink', !!S.eink);
    h.classList.toggle('k-legacy', !!S.legacy);
    h.classList.toggle('k-fixed', !!S.fixed);
    h.classList.toggle('k-paged', !state.scrolled && !S.fixed);
    h.classList.toggle('k-scrolled', state.scrolled);
    h.classList.toggle('k-v', state.vertical);
    h.classList.toggle('k-h', !state.vertical);
    if (S.fixed) fit();
    openingMargins();
    state.nodes = null;
    void (state.axis === 'y' ? scroller().scrollHeight : scroller().scrollWidth); // lay out now
    state.pages = count();
  }

  // A margin above the first thing in the text (a chapter title's) is kept at the top of the
  // first column and dropped at the top of every other, so the columns would start at
  // different heights. The elements down the first branch lose theirs, in every column.
  function openingMargins() {
    var el = document.body && document.body.firstElementChild;
    while (el) {
      el.classList.add('k-first');
      var hasText = Array.prototype.some.call(el.childNodes, function (n) {
        return n.nodeType === 3 && /\S/.test(n.data);
      });
      if (hasText) break;
      el = el.firstElementChild;
    }
  }

  // A fixed-layout page at its own size, scaled into the window and centred.
  function fit() {
    var size = fixedSize(), h = document.documentElement;
    if (!size) return;
    var s = Math.min(vw() / size.w, vh() / size.h);
    var x = (vw() - size.w * s) / 2, y = (vh() - size.h * s) / 2;
    state.fixedScale = s;
    h.style.setProperty('width', size.w + 'px');
    h.style.setProperty('height', size.h + 'px');
    h.style.setProperty('transform-origin', '0 0');
    h.style.setProperty('transform', 'translate(' + x + 'px,' + y + 'px) scale(' + s + ')');
  }

  // ---- Geometry -----------------------------------------------------------------------

  function scroller() { return document.scrollingElement || document.documentElement; }
  // The layout viewport. innerWidth is the visual one, which mobile Chrome widens to show
  // the columns overflowing to the side.
  function vw() { return document.documentElement.clientWidth || window.innerWidth; }
  function vh() { return document.documentElement.clientHeight || window.innerHeight; }

  function pitch() { return state.axis === 'y' ? vh() : vw(); }
  // Where a client rect starts along the reading axis, in document pixels.
  function along(r) {
    if (state.axis === 'y') return r.top + window.scrollY;
    if (state.rtl) return vw() - r.right - window.scrollX;
    return r.left + window.scrollX;
  }
  function pageAt(px) { return Math.floor((px + 0.5) / pitch()); }

  function textNodes() {
    if (state.nodes) return state.nodes;
    var out = [], w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null, false), n;
    while ((n = w.nextNode())) {
      var p = n.parentElement;
      if (!p || /^(script|style|rt|rp|title)$/i.test(p.localName)) continue;
      if (!/\S/.test(n.data.replace(SHY, ''))) continue;
      rng.selectNodeContents(n);
      if (!rng.getClientRects().length) continue; // not rendered: a hidden footnote and the like
      out.push(n);
    }
    state.nodes = out;
    return out;
  }

  function charRect(node, i) {
    var len = node.data.length;
    for (var k = i; k < Math.min(len, i + 12); k++) {
      if (/\s|­/.test(node.data[k])) continue;
      rng.setStart(node, k); rng.setEnd(node, k + 1);
      var rs = rng.getClientRects();
      if (rs.length && (rs[0].width > 0 || rs[0].height > 0)) return rs[0];
    }
    // text-combine-upright runs and the like give empty character rects: the node's box,
    // then its element's.
    rng.selectNodeContents(node);
    var b = rng.getBoundingClientRect();
    if (b.width > 0 || b.height > 0) return b;
    b = node.parentElement ? node.parentElement.getBoundingClientRect() : null;
    return b && (b.width > 0 || b.height > 0) ? b : null;
  }
  function charAt(node, i) { var r = charRect(node, i); return r ? along(r) : -1; }
  function lastIdx(node) { var d = node.data, i = d.length - 1; while (i > 0 && /[\s­]/.test(d[i])) i--; return i; }
  function firstIdx(node) { var d = node.data, i = 0; while (i < d.length - 1 && /[\s­]/.test(d[i])) i++; return i; }

  function count() {
    if (S.fixed) return 1;
    var size = state.axis === 'y' ? scroller().scrollHeight : scroller().scrollWidth;
    if (state.scrolled) return Math.max(1, Math.ceil((size - 1) / pitch()));
    var nodes = textNodes(), last = 0;
    if (nodes.length) { var n = nodes[nodes.length - 1]; last = pageAt(charAt(n, lastIdx(n))); }
    // Pictures after the last text count too.
    var media = document.body ? document.body.querySelectorAll('img,svg,video') : [];
    for (var i = media.length - 1; i >= 0 && i >= media.length - 4; i--) {
      var r = media[i].getBoundingClientRect();
      if (r.width || r.height) last = Math.max(last, pageAt(along(r)));
    }
    return Math.max(1, last + 1);
  }

  // The first character at or after `px` along the reading axis.
  function firstFrom(px) {
    var nodes = textNodes(), lo = 0, hi = nodes.length - 1, ans = -1;
    while (lo <= hi) { // the first node whose last character is at or after px
      var mid = (lo + hi) >> 1;
      if (charAt(nodes[mid], lastIdx(nodes[mid])) >= px - 0.5) { ans = mid; hi = mid - 1; } else lo = mid + 1;
    }
    if (ans < 0) return null;
    var node = nodes[ans], a = firstIdx(node), b = lastIdx(node), off = b;
    while (a <= b) {
      var m = (a + b) >> 1, q = charAt(node, m);
      if (q >= px - 0.5 || q < 0) { off = m; b = m - 1; } else a = m + 1;
    }
    return { node: node, offset: off };
  }

  // ---- Positions ----------------------------------------------------------------------

  // Element path from <body>, then characters into that element's text, ruby included and
  // soft hyphens not counted (Windows and Linux insert them).
  function posOf(node, off) {
    var el = node.nodeType === 1 ? node : node.parentElement, path = [];
    for (var e = el; e && e !== document.body; e = e.parentElement) {
      var i = 0; for (var s = e.previousElementSibling; s; s = s.previousElementSibling) i++;
      path.unshift(i);
    }
    var count = 0;
    if (node.nodeType === 3) {
      var w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null, false), n;
      while ((n = w.nextNode()) && n !== node) count += n.data.replace(SHY, '').length;
      count += node.data.slice(0, off).replace(SHY, '').length;
    }
    return { spine: S.spine, path: path, offset: count };
  }

  function posText(p) { return p.spine + '/' + p.path.join('.') + '/' + p.offset; }
  function parsePos(t) {
    if (t && typeof t === 'object') return t;
    var parts = String(t).split('/');
    if (parts.length !== 3) return null;
    var path = parts[1] === '' ? [] : parts[1].split('.').map(Number);
    if (path.some(isNaN) || isNaN(+parts[2])) return null;
    return { spine: +parts[0], path: path, offset: +parts[2] };
  }

  // The node and raw offset a position names. A path that no longer fits the document
  // (the book was edited) falls back to the deepest element that still exists.
  function resolve(pos) {
    var el = document.body;
    if (!el) return null;
    for (var i = 0; i < pos.path.length; i++) {
      var next = el.children[pos.path[i]];
      if (!next) break;
      el = next;
    }
    var w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null, false), n, list = [], left = pos.offset;
    while ((n = w.nextNode())) list.push(n);
    for (var j = 0; j < list.length; j++) {
      n = list[j];
      var clean = n.data.replace(SHY, '');
      if (left < clean.length) {
        var raw = 0, seen = 0; // a clean offset back to a raw one
        while (raw < n.data.length && (seen < left || n.data[raw] === '­')) { if (n.data[raw] !== '­') seen++; raw++; }
        return { node: n, offset: raw };
      }
      left -= clean.length;
    }
    var lastN = list[list.length - 1];
    if (lastN) return { node: lastN, offset: lastIdx(lastN) };
    return { node: el, offset: 0 };
  }

  function pageOfTarget(t) {
    if (!t) return 0;
    var px;
    if (t.node.nodeType === 3) px = charAt(t.node, t.offset);
    else { var r = t.node.getBoundingClientRect(); px = r.width || r.height ? along(r) : -1; }
    return px < 0 ? 0 : pageAt(px);
  }

  function here() {
    if (S.fixed) return { spine: S.spine, path: [], offset: 0 };
    var px = state.scrolled ? (window.scrollY) : state.page * pitch();
    var f = firstFrom(px);
    return f ? posOf(f.node, f.offset) : { spine: S.spine, path: [], offset: 0 };
  }

  // ---- Turning ------------------------------------------------------------------------

  function show(p) {
    p = Math.max(0, Math.min(state.pages - 1, p));
    state.page = p;
    var v = p * pitch();
    // window.scrollTo, not scrollTop: WebKit ignores scrollTop in vertical-rl documents.
    if (state.axis === 'y') window.scrollTo(0, v);
    else window.scrollTo(state.rtl ? -v : v, 0);
  }

  function report(type, how) {
    var pos = state.anchor || here();
    var msg = { type: type, spine: S.spine, page: state.page, pages: state.pages, pos: pos, at: posText(pos) };
    if (how) msg.how = how;
    msg.last = state.page + 1 >= state.pages;
    return msg;
  }

  function turned(how) {
    state.anchor = here();
    var msg = report('page', how);
    post(msg);
    return msg;
  }

  function goTo(p, how) { show(p); return turned(how || 'jump'); }

  function next(how) {
    if (state.scrolled) { window.scrollBy(0, Math.round(vh() * 0.9)); return settle(how || 'key'); }
    if (state.page + 1 >= state.pages) { post({ type: 'end', spine: S.spine }); return null; }
    return goTo(state.page + 1, how || 'turn');
  }

  function prev(how) {
    if (state.scrolled) { window.scrollBy(0, -Math.round(vh() * 0.9)); return settle(how || 'key'); }
    if (state.page <= 0) { post({ type: 'start', spine: S.spine }); return null; }
    return goTo(state.page - 1, how || 'turn');
  }

  function goToPos(pos, how) {
    var p = parsePos(pos);
    if (!p) return null;
    var t = resolve(p);
    show(pageOfTarget(t));
    if (state.scrolled && t) scrollToTarget(t);
    // A jump to a stored place keeps that place as the anchor, not the page's first character.
    state.anchor = { spine: S.spine, path: p.path, offset: p.offset };
    var msg = report('page', how || 'jump');
    post(msg);
    return msg;
  }

  function scrollToTarget(t) {
    var r = t.node.nodeType === 3 ? charRect(t.node, t.offset) : t.node.getBoundingClientRect();
    if (r) window.scrollTo(0, Math.max(0, r.top + window.scrollY - S.top));
    state.page = pageAt(window.scrollY);
  }

  function goToFragment(id, how) {
    var el = id ? document.getElementById(id) : null;
    if (!el) return goTo(0, how);
    show(pageOfTarget({ node: el, offset: 0 }));
    if (state.scrolled) scrollToTarget({ node: el, offset: 0 });
    return turned(how || 'jump');
  }

  // A scroll that came to rest: the place moves.
  function settle(how) {
    clearTimeout(state.settling);
    state.settling = setTimeout(function () {
      state.page = pageAt(window.scrollY);
      turned(how || 'scroll');
    }, 250);
    return null;
  }

  // Settings changed or the window resized: lay out again and keep the anchor.
  function relayout(changes) {
    var anchor = state.anchor || here();
    if (changes) S = merge(S, typeof changes === 'string' ? JSON.parse(changes) : changes);
    apply();
    var t = resolve(anchor);
    show(pageOfTarget(t));
    if (state.scrolled && t) scrollToTarget(t);
    state.anchor = anchor;
    var msg = report('layout');
    delete msg.how;
    post(msg);
    return msg;
  }

  // ---- Input --------------------------------------------------------------------------

  var selTimer = 0;
  document.addEventListener('selectionchange', function () {
    clearTimeout(selTimer);
    selTimer = setTimeout(function () {
      var sel = document.getSelection();
      if (!sel || sel.isCollapsed || !sel.rangeCount) {
        if (state.selection) { state.selection = null; post({ type: 'unselected' }); }
        return;
      }
      var r = sel.getRangeAt(0), b = r.getBoundingClientRect();
      var msg = { type: 'selection', text: sel.toString().replace(SHY, ''), rect: { x: b.left, y: b.top, w: b.width, h: b.height } };
      msg.start = posOf(r.startContainer, r.startOffset);
      msg.end = posOf(r.endContainer, r.endOffset);
      state.selection = msg;
      post(msg);
    }, 120);
  }, true);

  function hrefOf(a) {
    var raw = a.getAttribute('href') || a.getAttributeNS('http://www.w3.org/1999/xlink', 'href');
    if (!raw) return null;
    try { return new URL(raw, document.baseURI).href; } catch (e) { return null; }
  }

  document.addEventListener('click', function (e) {
    var a = e.target && e.target.closest ? e.target.closest('a') : null;
    var href = a ? hrefOf(a) : null;
    if (a && href) {
      e.preventDefault(); e.stopPropagation();
      var url = new URL(href), self = new URL(location.href);
      var internal = url.origin === self.origin && url.protocol === self.protocol &&
        url.pathname.split('/').slice(0, 3).join('/') === self.pathname.split('/').slice(0, 3).join('/');
      var msg = { type: 'link', href: href, internal: internal };
      var sameDoc = internal && url.pathname === self.pathname;
      var type = (a.getAttributeNS('http://www.idpf.org/2007/ops', 'type') || a.getAttribute('epub:type') || '');
      if (sameDoc && url.hash) {
        var target = document.getElementById(decodeURIComponent(url.hash.slice(1)));
        if (/noteref/.test(type) && target) {
          msg.note = target.textContent.replace(SHY, '').trim().slice(0, 2000);
        } else {
          goToFragment(decodeURIComponent(url.hash.slice(1)), 'jump');
          return;
        }
      }
      post(msg);
      return;
    }
    var sel = document.getSelection();
    if (sel && !sel.isCollapsed) return;
    post({ type: 'tap', x: e.clientX / vw(), y: e.clientY / vh() });
  }, true);

  var KEYS = { ArrowLeft: 1, ArrowRight: 1, ArrowUp: 1, ArrowDown: 1, PageUp: 1, PageDown: 1, ' ': 1, Home: 1, End: 1 };
  document.addEventListener('keydown', function (e) {
    if (!KEYS[e.key] || e.metaKey || e.ctrlKey || e.altKey) return;
    if (!state.scrolled) e.preventDefault();
    post({ type: 'key', key: e.key });
  }, true);

  // Paged documents never scroll by themselves: wheels and stray scrolls snap back to the page.
  window.addEventListener('wheel', function (e) { if (!state.scrolled) e.preventDefault(); }, { passive: false, capture: true });
  window.addEventListener('scroll', function () {
    if (!state.ready) return;
    if (state.scrolled) { settle('scroll'); return; }
    var v = state.page * pitch(), at = state.axis === 'y' ? window.scrollY : Math.abs(window.scrollX);
    if (Math.abs(at - v) > 1) show(state.page);
  }, true);
  window.addEventListener('resize', function () {
    if (!state.ready) return;
    clearTimeout(state.resizing);
    state.resizing = setTimeout(function () { relayout(null); }, 100);
  });
  window.addEventListener('hashchange', function () { if (state.ready) post(start('jump')); });

  // ---- Start --------------------------------------------------------------------------

  function start(how) {
    var hash = decodeURIComponent(location.hash.replace(/^#/, ''));
    if (hash === 'kaeri:end') { show(state.pages - 1); state.anchor = here(); return report(how ? 'page' : 'ready', how); }
    if (hash.indexOf('kaeri:pos=') === 0) {
      var p = parsePos(hash.slice('kaeri:pos='.length));
      if (p) {
        var t = resolve(p);
        show(pageOfTarget(t));
        if (state.scrolled && t) scrollToTarget(t);
        state.anchor = { spine: S.spine, path: p.path, offset: p.offset };
        return report(how ? 'page' : 'ready', how);
      }
    }
    var el = hash && hash.indexOf('kaeri:') !== 0 ? document.getElementById(hash) : null;
    show(el ? pageOfTarget({ node: el, offset: 0 }) : 0);
    if (el && state.scrolled) scrollToTarget({ node: el, offset: 0 });
    state.anchor = here();
    return report(how ? 'page' : 'ready', how);
  }

  function begin() {
    S = setup();
    var failed = document.body ? document.body.getAttribute('data-kaeri-failed') : null;
    if (failed) post({ type: 'failed', spine: S.spine, why: failed });
    apply();
    state.ready = true;
    var msg = start(null);
    msg.vertical = state.vertical;
    msg.failed = failed || null;
    post(msg);
  }

  window.kaeriReader = {
    next: next, prev: prev, goTo: goTo, goToPos: goToPos, goToFragment: goToFragment,
    configure: relayout,
    state: function () { return report('state'); },
    // For tests and the hidden counting view.
    count: function () { return state.pages; },
    posOf: posOf, resolve: resolve, here: here
  };

  // Lay out only once stylesheets, images and fonts are in: the book's CSS sets the writing
  // mode, and pages measured earlier are wrong.
  function ready() {
    var go = function () { try { begin(); } catch (e) { post({ type: 'failed', why: 'script', detail: String(e) }); } };
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(go, go); else go();
  }
  if (document.readyState === 'complete') ready(); else window.addEventListener('load', ready);
})();
