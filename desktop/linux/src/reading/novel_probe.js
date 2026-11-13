// QA only: what WebKitGTK made of the paginator's page, posted as {type:'qa'} messages.
(function () {
  if (window.top !== window) return;
  if (!/^kaeri-book:/.test(location.href)) return;
  function post(o) { try { window.webkit.messageHandlers.kaeri.postMessage(JSON.stringify(o)); } catch (e) {} }
  function rect(e) { var r = e.getBoundingClientRect(); return [Math.round(r.left), Math.round(r.top), Math.round(r.width), Math.round(r.height)]; }
  function lastChar() {
    var b = document.body; if (!b) return null;
    var w = document.createTreeWalker(b, NodeFilter.SHOW_TEXT, null, false), n, last = null, first = null, nodes = 0;
    while ((n = w.nextNode())) { if (/\S/.test(n.data)) { if (!first) first = n; last = n; nodes++; } }
    if (!last) return null;
    var r = document.createRange(), i = last.data.replace(/\s+$/, '').length - 1;
    r.setStart(last, i); r.setEnd(last, i + 1);
    var rs = r.getClientRects();
    r.selectNodeContents(last);
    var all = r.getClientRects();
    return { nodes: nodes, len: last.data.length, rects: rs.length, at: rs.length ? [Math.round(rs[0].left), Math.round(rs[0].top)] : null,
      nodeRects: all.length, nodeFirst: all.length ? [Math.round(all[0].left), Math.round(all[0].top)] : null,
      nodeLast: all.length ? [Math.round(all[all.length - 1].left), Math.round(all[all.length - 1].top)] : null };
  }
  function q(tag) {
    try {
      var h = document.documentElement, b = document.body, cs = getComputedStyle(h), se = document.scrollingElement || h;
      var o = { type: 'qa', tag: tag, ct: document.contentType, cm: document.compatMode,
        iw: innerWidth, ih: innerHeight, cw: h.clientWidth, ch: h.clientHeight, sx: scrollX, sy: scrollY,
        vv: window.visualViewport ? [visualViewport.width, visualViewport.height] : null, dpr: devicePixelRatio,
        cls: h.className, cc: cs.columnCount, cwid: cs.columnWidth, cg: cs.columnGap, cf: cs.columnFill,
        hh: cs.height, hw: cs.width, mh: cs.maxHeight, ov: cs.overflow, pad: cs.padding, disp: cs.display, wm: cs.writingMode,
        pos: cs.position, bs: cs.boxSizing,
        sw: se.scrollWidth, sh: se.scrollHeight, hsw: h.scrollWidth, hsh: h.scrollHeight,
        hr: rect(h), br: b ? rect(b) : null,
        bcs: b ? [getComputedStyle(b).height, getComputedStyle(b).display, getComputedStyle(b).position, getComputedStyle(b).columnCount, getComputedStyle(b).overflow] : null,
        count: window.kaeriReader ? window.kaeriReader.count() : null,
        sup: CSS.supports('column-count', '2'), ua: navigator.userAgent,
        styleEl: !!document.getElementById('kaeri-reader-style'), sheets: document.styleSheets.length,
        vars: [cs.getPropertyValue('--k-cols'), cs.getPropertyValue('--k-page'), cs.getPropertyValue('--k-top')],
        last: lastChar() };
      var d = document.createElementNS('http://www.w3.org/1999/xhtml', 'div');
      d.style.cssText = 'position:fixed;left:0;top:0;width:200px;height:60px;column-width:100px;column-gap:0;column-fill:auto;overflow:hidden;font-size:16px;line-height:20px;visibility:hidden';
      d.textContent = new Array(200).join('word ');
      h.appendChild(d);
      var r = document.createRange(); r.selectNodeContents(d.firstChild); var rs = r.getClientRects();
      o.box = { sw: d.scrollWidth, cw: d.clientWidth, sh: d.scrollHeight, rects: rs.length,
        last: rs.length ? [Math.round(rs[rs.length - 1].left), Math.round(rs[rs.length - 1].top)] : null };
      h.removeChild(d);
      post(o);
    } catch (e) { post({ type: 'qa', tag: tag, err: String(e) }); }
  }
  window.kaeriQa = q;
  q('start');
  document.addEventListener('DOMContentLoaded', function () { q('dom'); });
  window.addEventListener('load', function () {
    q('load');
    [300, 1500, 4000, 9000].forEach(function (t) { setTimeout(function () { q('load+' + t); }, t); });
  });
  window.addEventListener('resize', function () { q('resize'); });
})();
