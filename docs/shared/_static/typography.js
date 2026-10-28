/* Paragraph-wide Knuth-Plass layout for English reading prose across the website.
   The original DOM is retained for resize and print. Emphasis, links, and inline code survive.
   Equations, code blocks, interactive content, and unsuitable measures use native CSS layout. */
(() => {
  "use strict";
  const article = document.querySelector(".gd-article, .landing");
  const language = (document.documentElement.lang || "en").toLowerCase();
  if (!article || !/^en(?:-|$)/.test(language) || typeof Typeset === "undefined" ||
      typeof Typeset.linebreak !== "function" || !Typeset.LinkedList ||
      typeof Hypher === "undefined" || !Hypher.en) return;
  const hypher = new Hypher(Hypher.en);
  const breaker = Typeset.linebreak;
  const canvas = document.createElement("canvas").getContext("2d");
  if (!canvas) return;
  const originals = new Map();
  const selector = [
    "section > p",
    ".desc-content > p",
    "li > p",
    "dd > p",
    ".admonition > p:not(.admonition-title)",
    "p.lead",
    ".lead > p",
    ".pillar > p",
    ".split-copy > p",
    ".book-copy > p"
  ].join(", ");
  const excluded = [
    "table",
    ".toctree-wrapper",
    "figure",
    ".actions",
    ".text-links",
    ".eyebrow",
    ".rubric",
    ".hero-note",
    ".art-caption"
  ].join(", ");
  const paragraphs = [...article.querySelectorAll(selector)].filter(p => {
    const length = p.textContent.length;
    return length >= 65 && length <= 5000 &&
      !p.matches(excluded) &&
      !p.closest(excluded) &&
      !p.querySelector(".math, mjx-container, img, br, sup, sub, [id], [role], [tabindex], button") &&
      [...p.querySelectorAll("*")].every(e => /^(A|EM|STRONG|SPAN|CODE)$/.test(e.tagName));
  });

  function restore(p) {
    const original = originals.get(p);
    if (original) p.replaceChildren(...original);
    delete p.dataset.layout;
  }

  function textMap(p) {
    const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
    const map = [];
    let text = "", node;
    while ((node = walker.nextNode())) {
      map.push({node, start: text.length, end: text.length + node.length});
      text += node.textContent;
    }
    return {map, text};
  }

  function point(map, offset) {
    const item = map.find(a => offset >= a.start && offset < a.end) || map.at(-1);
    return {node: item.node, offset: Math.min(offset - item.start, item.node.length)};
  }

  function fontOf(element) {
    const style = getComputedStyle(element);
    return `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
  }

  function fontAt(map, offset) {
    return fontOf(point(map, offset).node.parentElement);
  }

  // The inline code elements that the text from start to end touches.
  function codesIn(map, start, end) {
    return new Set(map.filter(item => item.end > start && item.start < end)
      .map(item => item.node.parentElement.closest("code")).filter(Boolean));
  }

  // The padding and border of the code boxes that open (left) or close (right) within the
  // text from start to end. Where a line breaks inside a code box, render() removes them
  // at the cut, so a box counts each side once, wherever its lines break.
  function codeEdges(map, start, end) {
    let left = 0, right = 0;
    for (const code of codesIn(map, start, end)) {
      const items = map.filter(item => item.node.parentElement.closest("code") === code);
      const style = getComputedStyle(code);
      if (items[0].start >= start) {
        left += (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.borderLeftWidth) || 0);
      }
      if (items.at(-1).end <= end) {
        right += (parseFloat(style.paddingRight) || 0) + (parseFloat(style.borderRightWidth) || 0);
      }
    }
    return [left, right];
  }

  function width(text, font) {
    canvas.font = font;
    return canvas.measureText(text).width;
  }

  function pushSyllables(part, partStart, inCode, font, extra, nodes) {
    const letters = inCode
      ? null
      : part.match(/^([\u201c\u2018(]*)([a-zA-Z]{6,})((?:['\u2019][a-zA-Z]+)?[.,;:!?"\u201d\u2019)\]]*-?)$/);
    const fragments = letters ? hypher.hyphenate(letters[2]) : [part];
    if (letters) {
      fragments[0] = letters[1] + fragments[0];
      fragments[fragments.length - 1] += letters[3];
    }
    let offset = partStart;
    for (let j = 0; j < fragments.length; j++) {
      const fragment = fragments[j];
      const boxWidth = width(fragment, font) + (j === 0 ? extra : 0);
      const box = breaker.box(boxWidth, fragment);
      nodes.push(box);
      offset += fragment.length;
      if (j < fragments.length - 1) {
        const penalty = breaker.penalty(width("\u2010", font), 80, 1);
        penalty.offset = offset;
        penalty.trimEnd = offset;
        nodes.push(penalty);
      }
    }
  }

  function nodesFor(text, map, space) {
    const nodes = [];
    const words = [...text.matchAll(/\S+/g)];
    for (let i = 0; i < words.length; i++) {
      const word = words[i][0], start = words[i].index, end = start + word.length;
      // A word that runs into inline code, such as (path/to/file.md, is code: it is not
      // hyphenated, and its box carries the code's padding and border.
      const inCode = codesIn(map, start, end).size > 0;
      const font = fontAt(map, start);
      const [left, right] = inCode ? codeEdges(map, start, end) : [0, 0];
      // Break after a hyphen between letters only, and keep every character, so that
      // arrows such as -> and options such as --flag stay whole.
      const parts = inCode ? [word] : word.split(/(?<=\p{L}-)(?=\p{L})/u);
      let partOffset = start;
      for (let p = 0; p < parts.length; p++) {
        const part = parts[p];
        const extra = (p === 0 ? left : 0) + (p === parts.length - 1 ? right : 0);
        pushSyllables(part, partOffset, inCode, font, extra, nodes);
        partOffset += part.length;
        if (p < parts.length - 1 && part.endsWith("-")) {
          const penalty = breaker.penalty(0, 50, 1);
          penalty.offset = partOffset;
          penalty.trimEnd = partOffset;
          nodes.push(penalty);
        }
      }
      const last = i === words.length - 1;
      // A space inside inline code is as wide as that font's space. CSS stretches and
      // shrinks every space of a line alike, so all glue gets the body space's flexibility.
      const gap = !last && codesIn(map, end, end + 1).size ? width(" ", fontAt(map, end)) : space;
      const glue = breaker.glue(last ? 0 : gap, last ? breaker.infinity : space / 2,
        last ? 0 : space / 3);
      glue.offset = last ? text.length : words[i + 1].index;
      glue.trimEnd = end;
      nodes.push(glue);
    }
    const final = breaker.penalty(0, -breaker.infinity, 0);
    final.offset = text.length;
    final.trimEnd = text.length;
    nodes.push(final);
    return nodes;
  }

  function lineFragment(map, start, end) {
    const range = document.createRange();
    const from = point(map, start), to = point(map, end);
    range.setStart(from.node, from.offset);
    range.setEnd(to.node, to.offset);
    return range.cloneContents();
  }

  function render(p, map, text, nodes, breaks, space) {
    const fragment = document.createDocumentFragment();
    const codeOf = offset => offset >= 0 && offset < text.length
      ? point(map, offset).node.parentElement.closest("code") : null;
    let start = 0, cut = false;
    for (let i = 1; i < breaks.length; i++) {
      const choice = breaks[i], node = nodes[choice.position];
      const trimEnd = node.trimEnd ?? node.offset;
      const line = document.createElement("span");
      line.className = "gd-typeset-line";
      line.append(lineFragment(map, start, trimEnd));
      if (node.type === "penalty" && node.penalty > 0 && node.width > 0) {
        line.dataset.hyphen = "";
      }
      // A code box that the break cuts in two loses its padding and border at the cut.
      if (cut) line.querySelector("code")?.classList.add("gd-code-cut-start");
      cut = codeOf(trimEnd - 1) !== null && codeOf(trimEnd - 1) === codeOf(node.offset);
      if (cut) [...line.querySelectorAll("code")].at(-1)?.classList.add("gd-code-cut-end");
      // CSS justification stretches the spaces of a line but never shrinks them, so a
      // line that the breaker set tighter than its natural width gets negative spacing.
      // That includes the last line, which CSS does not justify at all.
      if (choice.ratio < 0) {
        line.style.wordSpacing = `${choice.ratio * space / 3}px`;
      }
      if (trimEnd < node.offset) {
        const sep = document.createElement("span");
        sep.className = "gd-line-space";
        sep.textContent = text.slice(trimEnd, node.offset);
        line.append(sep);
      }
      fragment.append(line);
      start = node.offset;
    }
    // Layout is optional: never replace the source with an incomplete result.
    if (start !== text.length || fragment.textContent !== text) return;
    p.replaceChildren(fragment);
    p.dataset.layout = "knuth-plass";
  }

  function typeset(p) {
    restore(p);
    const measure = p.clientWidth;
    if (measure < 220) return;
    const {map, text} = textMap(p);
    if (!map.length || !text.trim()) return;
    // The paragraph's own font, not that of its first word, which may be emphasis.
    const space = width(" ", fontOf(p));
    const nodes = nodesFor(text, map, space);
    if (nodes.length > 1800) return;
    const breaks = breaker(nodes, [measure - 1], {tolerance: 2, maxTolerance: 4,
      demerits: {line: 10, flagged: 400, fitness: 3000}});
    if (breaks.length < 3 || breaks.at(-1).position !== nodes.length - 1) return;
    if (breaks.slice(1, -1).some(b => b.ratio > 2.5 || b.ratio < -0.95)) return;
    if (!originals.has(p)) originals.set(p, [...p.childNodes]);
    render(p, map, text, nodes, breaks, space);
  }

  let scheduled = false, printing = false;
  function run() {
    scheduled = false;
    if (printing) return;
    for (const p of paragraphs) typeset(p);
  }
  function schedule() {
    if (scheduled || printing) return;
    scheduled = true;
    requestAnimationFrame(run);
  }
  run();
  document.fonts.ready.then(schedule);
  let previousWidth = 0;
  const resized = entries => {
    const measure = entries[0].contentRect.width;
    if (Math.abs(measure - previousWidth) > 0.5) {
      previousWidth = measure;
      schedule();
    }
  };
  if (typeof ResizeObserver !== "undefined") new ResizeObserver(resized).observe(article);
  else addEventListener("resize", schedule);
  addEventListener("beforeprint", () => {
    printing = true;
    for (const p of paragraphs) restore(p);
  });
  addEventListener("afterprint", () => { printing = false; schedule(); });
  // Copy the selected source text, without visual line divisions or inserted hyphens.
  addEventListener("copy", event => {
    const selection = getSelection();
    if (!selection || selection.isCollapsed || !selection.rangeCount) return;
    const paragraph = node => (node.nodeType === Node.TEXT_NODE ? node.parentElement : node)
      ?.closest('p[data-layout="knuth-plass"]');
    const from = paragraph(selection.anchorNode), to = paragraph(selection.focusNode);
    if (!from || from !== to || !event.clipboardData) return;
    event.clipboardData.setData("text/plain", selection.getRangeAt(0).cloneContents().textContent);
    event.preventDefault();
  });
})();
