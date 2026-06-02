/* ============================================================
   S-Bank panel kit — the Sam-e visual system as a renderer.
   Paths-only SVG (nanosvg-safe): rect / line / circle / path only.
   Exposes window.SAMPanel { PALETTE, STYLES, SPECS, build, buildOverlay }.
   ============================================================ */
(function (root) {
  var F = root.SAMFont;

  // ---- brand kit ------------------------------------------------------------
  var HP_MM = 5.08, PANEL_H = 128.5;
  var C = {
    ink: "#0B080B", paper: "#F5F5F1", gray: "#A6A6A6",
    orange: "#FF5A00", yellow: "#FFC400", cyan: "#19D2E5", eblue: "#1C46FF",
    engrave: "#ECE7DD",          // NASA-white lettering
    hair: "#F5F5F1"              // structure lines (drawn at low opacity)
  };

  // Finishes: the environment flips, the engraving flips, signal-state accents stay
  // in-family but adapt for contrast on light aluminium (still "voltage, not a rainbow").
  var THEMES = {
    black: {
      id: "black", name: "JET BLACK",
      bg: ["#100b12", "#0b070b", "#08060a"],
      ink: "#ECE7DD", hair: "#F5F5F1", gray: "#A6A6A6",
      well: "#14101a", wellOp: 0.9, wellStrokeOp: 0.16,
      orange: "#FF5A00", yellow: "#FFC400", cyan: "#19D2E5", eblue: "#1C46FF"
    },
    silver: {
      id: "silver", name: "BRUSHED SILVER",
      bg: ["#e2e3e6", "#d2d3d7", "#bfc0c5"],
      ink: "#17141c", hair: "#2b2733", gray: "#6c6d72",
      well: "#b4b5ba", wellOp: 0.9, wellStrokeOp: 0.5,
      orange: "#E8500A", yellow: "#A87400", cyan: "#0E93A6", eblue: "#1C46FF"
    }
  };
  var TH = THEMES.black;   // current finish; build() sets this before rendering

  // state = meaning. one hue, one job. never a rainbow.
  function accentFor(c) {
    if (c.kind === "out") return TH.orange;                // signal leaving / active
    if (c.kind === "light") return TH.yellow;              // energy / peak (the strike)
    if (c.kind === "trim") return TH.cyan;                 // information / control (CV)
    if (c.kind === "switch") return TH.eblue;              // depth / unknown (imperfection)
    if (c.kind === "in") {
      if (/HIT/.test(c.label)) return TH.yellow;           // trigger fires the strike
      if (/DEC|CTL|CV/.test(c.label)) return TH.cyan;      // CV in
    }
    return TH.ink;                                          // everything else = environment
  }

  // footprints (mm radius / half-size) — used for wells AND the overlay
  var FP = { knob: 6.4, knob_sm: 4.6, trim: 2.6, in: 3.15, out: 3.15, light: 1.4, switch: [2.6, 5.4] };

  // ---- tiny svg helpers -----------------------------------------------------
  var n = function (v) { return (+v).toFixed(2); };
  function line(x1, y1, x2, y2, col, w, op) {
    return '<line x1="' + n(x1) + '" y1="' + n(y1) + '" x2="' + n(x2) + '" y2="' + n(y2) +
      '" stroke="' + col + '" stroke-width="' + (w || 0.25) + '" stroke-opacity="' + (op == null ? 1 : op) + '" stroke-linecap="round"/>';
  }
  function circle(cx, cy, r, o) {
    o = o || {};
    return '<circle cx="' + n(cx) + '" cy="' + n(cy) + '" r="' + n(r) + '" fill="' + (o.fill || "none") +
      '" fill-opacity="' + (o.fillOp == null ? 1 : o.fillOp) + '" stroke="' + (o.stroke || "none") +
      '" stroke-width="' + (o.w || 0.25) + '" stroke-opacity="' + (o.op == null ? 1 : o.op) + '"/>';
  }
  function rrect(x, y, w, h, r, o) {
    o = o || {};
    return '<rect x="' + n(x) + '" y="' + n(y) + '" width="' + n(w) + '" height="' + n(h) + '" rx="' + n(r) +
      '" fill="' + (o.fill || "none") + '" fill-opacity="' + (o.fillOp == null ? 1 : o.fillOp) +
      '" stroke="' + (o.stroke || "none") + '" stroke-width="' + (o.w || 0.25) + '" stroke-opacity="' + (o.op == null ? 1 : o.op) + '"/>';
  }
  function polar(cx, cy, r, deg) { var a = deg * Math.PI / 180; return [cx + r * Math.cos(a), cy + r * Math.sin(a)]; }
  function arc(cx, cy, r, a0, a1, large, sweep, col, w, op) {
    var p0 = polar(cx, cy, r, a0), p1 = polar(cx, cy, r, a1);
    return '<path d="M' + n(p0[0]) + ',' + n(p0[1]) + ' A' + n(r) + ',' + n(r) + ' 0 ' + large + ' ' + sweep + ' ' +
      n(p1[0]) + ',' + n(p1[1]) + '" fill="none" stroke="' + col + '" stroke-width="' + (w || 0.25) +
      '" stroke-opacity="' + (op == null ? 1 : op) + '" stroke-linecap="round"/>';
  }
  function txt(s, x, y, size, col, anchor, weight, tracking) { return F.textPaths(s, x, y, size, col, anchor, weight, tracking); }

  // ---- composite motifs -----------------------------------------------------
  // 270deg instrument dial, gap at bottom: guard ring + graduated ticks + top index + min/max caps
  function gauge(cx, cy, r, col, op, thin, caps) {
    var o = [], A0 = 135, SWEEP = 270;
    // faint outer guard ring
    o.push(arc(cx, cy, r + 1.0, A0, 45, 1, 1, col, 0.16, op * 0.5));
    // main scale arc
    o.push(arc(cx, cy, r, A0, 45, 1, 1, col, thin ? 0.18 : 0.22, op));
    // graduated ticks (11 across the sweep; every 5th is major)
    var N = 10;
    for (var i = 0; i <= N; i++) {
      var a = A0 + (i / N) * SWEEP;
      var major = (i % 5 === 0);
      var ri = r - (major ? 1.4 : 0.7), ro = r + 0.12;
      var p0 = polar(cx, cy, ri, a), p1 = polar(cx, cy, ro, a);
      o.push(line(p0[0], p0[1], p1[0], p1[1], col, major ? 0.26 : 0.15, op + (major ? 0.18 : 0.04)));
    }
    // top index pointer (12 o'clock reference), reaching inward toward the cap
    var ti = polar(cx, cy, r - 2.1, 270), tb = polar(cx, cy, r + 0.7, 270);
    o.push(line(ti[0], ti[1], tb[0], tb[1], col, 0.3, op + 0.32));
    // min / max end caps — skipped when text descriptors already mark the extremes
    if (caps !== false) {
      var e0 = polar(cx, cy, r, A0), e1 = polar(cx, cy, r, 45);
      o.push(circle(e0[0], e0[1], 0.45, { fill: col, fillOp: op + 0.25 }));
      o.push(circle(e1[0], e1[1], 0.45, { fill: col, fillOp: op + 0.25 }));
    }
    return o.join("");
  }
  // recessed well behind a component
  function well(cx, cy, r) {
    return circle(cx, cy, r, { fill: TH.well, fillOp: TH.wellOp, stroke: TH.hair, w: 0.2, op: TH.wellStrokeOp });
  }
  // concentric "ringing body" pulses around the openness LED
  function ping(cx, cy, scale) {
    var o = [], rings = [[2.3, 0.5], [3.9, 0.3], [5.3, 0.17]];
    rings.forEach(function (rr) { o.push(circle(cx, cy, rr[0] * scale, { stroke: TH.yellow, w: 0.22, op: rr[1] })); });
    o.push(circle(cx, cy, 1.25, { fill: TH.yellow, fillOp: 0.85 }));   // LED stand-in (Rack draws real LED on top)
    return o.join("");
  }
  // L brackets at the corners of a bay
  function brackets(x0, y0, x1, y1, col, op, len) {
    len = len || 2.2;
    var o = [];
    [[x0, y0, 1, 1], [x1, y0, -1, 1], [x0, y1, 1, -1], [x1, y1, -1, -1]].forEach(function (b) {
      o.push(line(b[0], b[1], b[0] + len * b[2], b[1], col, 0.22, op));
      o.push(line(b[0], b[1], b[0], b[1] + len * b[3], col, 0.22, op));
    });
    return o.join("");
  }
  // attack/decay envelope glyph — the module's own signature curve
  function envelope(x, y, w, h, col, op) {
    var pk = x + w * 0.16;
    var d = "M" + n(x) + "," + n(y + h) +
      " L" + n(pk) + "," + n(y) +
      " C" + n(pk + w * 0.18) + "," + n(y + h * 0.15) + " " + n(pk + w * 0.34) + "," + n(y + h * 0.92) + " " + n(x + w) + "," + n(y + h);
    return '<path d="' + d + '" fill="none" stroke="' + col + '" stroke-width="0.3" stroke-opacity="' + op +
      '" stroke-linecap="round" stroke-linejoin="round"/>' +
      line(x, y + h, x + w, y + h, col, 0.18, op * 0.6);
  }

  // ---- the renderer ---------------------------------------------------------
  function build(spec, styleId, themeId) {
    var S = STYLES_BY_ID[styleId] || STYLES[0];
    TH = THEMES[themeId] || THEMES.black;
    var w = spec.hp * HP_MM, H = PANEL_H;
    var o = [];
    o.push('<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="' + n(w) + 'mm" height="' + H + 'mm" viewBox="0 0 ' + n(w) + ' ' + H + '">');
    o.push('<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="' + H + '" gradientUnits="userSpaceOnUse">' +
      '<stop offset="0" stop-color="' + TH.bg[0] + '"/><stop offset="0.5" stop-color="' + TH.bg[1] + '"/><stop offset="1" stop-color="' + TH.bg[2] + '"/></linearGradient></defs>');

    // 1. environment
    o.push('<rect x="0" y="0" width="' + n(w) + '" height="' + H + '" fill="url(#bg)"/>');
    o.push(rrect(0.5, 0.5, w - 1.0, H - 1.0, 1.6, { stroke: TH.hair, w: 0.3, op: 0.5 }));
    o.push(rrect(1.4, 1.4, w - 2.8, H - 2.8, 1.2, { stroke: TH.hair, w: 0.18, op: 0.14 }));
    // corner registration crosshairs
    [[4, 4], [w - 4, 4], [4, H - 4], [w - 4, H - 4]].forEach(function (c) {
      o.push(line(c[0] - 1.5, c[1], c[0] + 1.5, c[1], TH.hair, 0.22, 0.42));
      o.push(line(c[0], c[1] - 1.5, c[0], c[1] + 1.5, TH.hair, 0.22, 0.42));
    });

    // detect channel columns from knob x positions
    var cols = []; spec.comps.forEach(function (c) { if (c.kind === "knob" && cols.indexOf(c.x) < 0) cols.push(c.x); });
    cols.sort(function (a, b) { return a - b; });
    var mid = w / 2;

    // 2. structure: ruler / mirror rail / bays
    if (S.ruler) {
      for (var yy = 16; yy <= 120; yy += 5) {
        var major = (Math.round(yy) % 10 === 0);
        o.push(line(2.0, yy, 2.0 + (major ? 1.8 : 1.0), yy, TH.hair, 0.18, 0.18));
        o.push(line(w - 2.0, yy, w - 2.0 - (major ? 1.8 : 1.0), yy, TH.hair, 0.18, 0.18));
      }
    }
    if (S.rail && cols.length === 2) {
      o.push(line(mid, 15.5, mid, 102, TH.hair, 0.18, 0.16));
      for (var ry = 20; ry <= 100; ry += 10) o.push(line(mid - 0.7, ry, mid + 0.7, ry, TH.hair, 0.18, 0.16));
      // mirror-axis diamond at vertical center
      var dC = 58;
      o.push('<path d="M' + n(mid) + ',' + n(dC - 1.1) + ' L' + n(mid + 1.1) + ',' + n(dC) + ' L' + n(mid) + ',' + n(dC + 1.1) + ' L' + n(mid - 1.1) + ',' + n(dC) + ' Z" fill="none" stroke="' + TH.hair + '" stroke-width="0.18" stroke-opacity="0.22"/>');
    }
    if (S.zones === "boxes") {
      cols.forEach(function (cx) { o.push(rrect(cx - 11, 15.5, 22, 105.5, 1.4, { stroke: TH.hair, w: 0.2, op: 0.13 })); });
    } else if (S.zones === "brackets") {
      cols.forEach(function (cx) { o.push(brackets(cx - 11, 15.5, cx + 11, 121, TH.hair, 0.3, 2.4)); });
    }

    // 3. masthead (width-aware: wide panels carry full telemetry, narrow ones simplify)
    var mx = w < 50 ? 5 : 7;
    var narrow = w < 50;
    o.push(txt("S-", mx, 5.4, narrow ? 1.9 : 2.1, TH.orange, "start", 0.26));
    o.push(txt("BANK", mx + F.textWidth("S-", narrow ? 1.9 : 2.1) + 0.4, 5.4, narrow ? 1.9 : 2.1, TH.ink, "start", 0.22));
    o.push(txt(spec.title, mx, 10.6, narrow ? 3.4 : 3.6, TH.ink, "start", 0.32));
    if (narrow) {
      o.push(txt(spec.hp + "HP", w - mx, 5.4, 1.4, TH.gray, "end", 0.18));
      o.push(txt("S-" + spec.serial, w - mx, 10.6, 1.3, TH.gray, "end", 0.16));
    } else {
      o.push(txt(spec.hp + "HP / S-" + spec.serial, w - mx, 5.4, 1.5, TH.gray, "end", 0.18));
      if (spec.sub) {
        var parts = spec.sub.split("|");
        o.push(txt(parts[0].trim(), w - mx, 8.0, 1.4, TH.gray, "end", 0.16));
        if (parts[1]) o.push(txt(parts[1].trim(), w - mx, 10.4, 1.4, TH.gray, "end", 0.16));
      }
    }
    o.push(line(mx, 12.2, w - mx, 12.2, TH.hair, 0.25, 0.34));
    if (S.envelope) o.push(envelope(mid - 7, 14.2, 14, 3.2, TH.cyan, 0.5));

    // user dividers
    (spec.dividers || []).forEach(function (y) { o.push(line(mx, y, w - mx, y, TH.hair, 0.22, 0.2)); });

    // 4. signal trace (MkIII) — inputs converge UP into the gate (LED), signal flows down to OUT
    if (S.trace && cols.length) {
      cols.forEach(function (cx) {
        o.push(line(cx, 84, cx, 115.5, TH.ink, 0.22, 0.34));      // gate -> OUT spine
        o.push(line(cx - 9, 93, cx, 85.5, TH.ink, 0.2, 0.34));    // IN (audio) feeds the gate, from the left
        o.push(line(cx + 9, 93, cx, 85.5, TH.yellow, 0.22, 0.44));    // HIT (trigger) fires the gate, from the right
        o.push('<path d="M' + n(cx - 1.0) + ',114.0 L' + n(cx) + ',116.0 L' + n(cx + 1.0) + ',114.0" fill="none" stroke="' + TH.orange + '" stroke-width="0.3" stroke-linecap="round" stroke-linejoin="round"/>'); // signal leaves
      });
    }
    // big dim S- watermark (MkIII)
    if (S.watermark) {
      o.push(txt("S-", mid, 78, 26, TH.ink, "middle", 0.5, 0.6).replace('stroke-opacity', 'data-x').replace('"/>', '" stroke-opacity="0.05"/>'));
    }

    // 5. CV grouping ties (cyan) — link the DEC/CTL trim pairs
    var trims = spec.comps.filter(function (c) { return c.kind === "trim"; });
    cols.forEach(function (cx) {
      var pair = trims.filter(function (t) { return Math.abs(t.y - 73) < 1 && Math.abs(t.x - cx) <= 9; });
      if (pair.length === 2) {
        o.push(line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, TH.cyan, 0.18, 0.32));
        o.push(txt("CV", cx, 69.4, 1.4, TH.cyan, "middle", 0.16));
      }
    });

    // 6. per-component: wells, gauges, ping, accent rings
    spec.comps.forEach(function (c) {
      var acc = accentFor(c);
      if (c.kind === "knob" || c.kind === "knob_sm") {
        var fr = FP[c.kind];
        o.push(well(c.x, c.y, fr - 0.6));
        o.push(gauge(c.x, c.y, fr + 0.9, TH.hair, 0.28, S.gaugesThin, !(c.lo || c.hi)));
        if (c.lo || c.hi) {                      // scale-end descriptors just outside the arc ends
          var capr = (fr + 0.9) * 0.707, ex = capr + 0.4, ey = c.y + capr + 1.2;
          if (c.lo) o.push(txt(c.lo, c.x - ex, ey, 1.1, TH.gray, "end", 0.15));
          if (c.hi) o.push(txt(c.hi, c.x + ex, ey, 1.1, TH.gray, "start", 0.15));
        }
      } else if (c.kind === "trim") {
        o.push(well(c.x, c.y, FP.trim + 0.5));
      } else if (c.kind === "in" || c.kind === "out") {
        o.push(well(c.x, c.y, FP[c.kind] + 0.5));
        if (c.kind === "out") o.push(circle(c.x, c.y, FP.out + 0.75, { stroke: TH.orange, w: 0.28, op: 0.55 }));
        if (c.kind === "in" && acc !== TH.ink) o.push(circle(c.x, c.y, FP.in + 0.7, { stroke: acc, w: 0.22, op: 0.4 }));
      } else if (c.kind === "light") {
        o.push(ping(c.x, c.y, S.ping));
      } else if (c.kind === "switch") {
        var sw = FP.switch;
        o.push(rrect(c.x - sw[0] / 2, c.y - sw[1] / 2, sw[0], sw[1], 0.8, { fill: TH.well, fillOp: TH.wellOp, stroke: TH.eblue, w: 0.22, op: 0.5 }));
        // scatter glyph = imperfection / depth
        o.push(circle(c.x - 3.2, c.y - 1.0, 0.26, { fill: TH.eblue, fillOp: 0.7 }));
        o.push(circle(c.x - 3.7, c.y + 0.8, 0.26, { fill: TH.eblue, fillOp: 0.5 }));
        o.push(circle(c.x + 3.4, c.y + 0.4, 0.26, { fill: TH.eblue, fillOp: 0.6 }));
      }
    });

    // 7. labels (above components by default; flip below if they'd hit the masthead)
    spec.comps.forEach(function (c) {
      if (!c.label) return;
      var acc = accentFor(c);
      var size = (c.kind === "knob" || c.kind === "knob_sm") ? 1.9 : 1.6;
      var fpR = (FP[c.kind] && FP[c.kind].length ? FP[c.kind][1] / 2 : FP[c.kind] || 5);
      var ly = c.y - fpR - 1.4;
      if (ly < 14.5) ly = c.y + fpR + 2.6;   // too close to masthead -> label below
      o.push(txt(c.label, c.x, ly, size, acc, "middle", 0.18));
    });

    // 8. free notes (nt.col may be a theme key like "gray"/"eblue", or a literal colour)
    (spec.notes || []).forEach(function (nt) {
      var col = (nt.col && TH[nt.col]) || nt.col || TH.gray;
      o.push(txt(nt.text, nt.x, nt.y, nt.size || 1.6, col, nt.anchor || "middle", 0.16));
    });

    // 9. footer telemetry (width-aware)
    o.push(line(mx, H - 6.2, w - mx, H - 6.2, TH.hair, 0.25, 0.34));
    var fb = H - 4.0;
    if (narrow) {
      var sw0 = F.textWidth("SAM-E", 1.6);
      o.push(circle(mid - sw0 / 2 - 1.6, fb - 0.55, 0.8, { fill: TH.orange, fillOp: 0.95 }));
      o.push(txt("SAM-E", mid, fb, 1.6, TH.ink, "middle", 0.2));
    } else {
      o.push(txt("S- " + spec.hp + "HP", mx, fb, 1.45, TH.gray, "start", 0.16));
      o.push(txt("SAM-E", mid, fb, 1.6, TH.ink, "middle", 0.2));
      var statusW = F.textWidth("SIGNAL STABLE", 1.4);
      o.push(circle(w - mx - statusW - 2.0, fb - 0.55, 0.85, { fill: TH.orange, fillOp: 0.95 }));
      o.push(txt("SIGNAL STABLE", w - mx, fb, 1.4, TH.gray, "end", 0.16));
    }

    o.push('</svg>');
    return o.join("\n");
  }

  // component-footprint overlay (proves knob/jack clearance when superimposed)
  function buildOverlay(spec) {
    var w = spec.hp * HP_MM, H = PANEL_H, o = [];
    o.push('<svg xmlns="http://www.w3.org/2000/svg" width="' + n(w) + 'mm" height="' + H + 'mm" viewBox="0 0 ' + n(w) + ' ' + H + '">');
    spec.comps.forEach(function (c) {
      var fp = FP[c.kind];
      if (fp && fp.length) {
        o.push(rrect(c.x - fp[0] / 2, c.y - fp[1] / 2, fp[0], fp[1], 0.6, { fill: C.cyan, fillOp: 0.16, stroke: C.cyan, w: 0.3, op: 0.9 }));
      } else if (fp) {
        o.push(circle(c.x, c.y, fp, { fill: C.cyan, fillOp: 0.16, stroke: C.cyan, w: 0.3, op: 0.9 }));
        o.push(line(c.x - 0.8, c.y, c.x + 0.8, c.y, C.cyan, 0.2, 0.9));
        o.push(line(c.x, c.y - 0.8, c.x, c.y + 0.8, C.cyan, 0.2, 0.9));
      }
    });
    o.push('</svg>');
    return o.join("\n");
  }

  // ---- styles & specs -------------------------------------------------------
  var STYLES = [
    { id: "mk1", name: "MK I — INSTRUMENT", tag: "Restraint. Engraved gauges, mirror axis, ringing body.", zones: "brackets", rail: true, gaugesThin: false, ping: 0.85, trace: false, envelope: false, ruler: false, watermark: false },
    { id: "mk2", name: "MK II — TELEMETRY", tag: "Denser. Bays, edge ruler, the module's own decay curve.", zones: "boxes", rail: true, gaugesThin: false, ping: 0.72, trace: false, envelope: true, ruler: true, watermark: false },
    { id: "mk3", name: "MK III — SIGNAL TRACE", tag: "Boldest. The audio path drawn as signal, S- watermark.", zones: "none", rail: false, gaugesThin: true, ping: 0.85, trace: true, envelope: false, ruler: false, watermark: true }
  ];
  var STYLES_BY_ID = {}; STYLES.forEach(function (s) { STYLES_BY_ID[s.id] = s; });

  function strikeSpec() {
    var cols = [20.32, 60.96], comps = [], notes = [];
    cols.forEach(function (x, i) {
      var a = i === 0 ? "A" : "B";
      // channel header sits in the outer-top corner of the bay, not over the OPEN knob
      notes.push({ x: i === 0 ? x - 11 : x + 11, y: 14.6, text: "CH " + a, size: 1.5, col: "gray", anchor: i === 0 ? "start" : "end" });
      comps.push({ kind: "knob", x: x, y: 24, eid: a + "_OPEN_PARAM", label: "OPEN", lo: "SHUT", hi: "OPEN" });
      comps.push({ kind: "knob", x: x, y: 42, eid: a + "_DECAY_PARAM", label: "DECAY", lo: "FAST", hi: "SLOW" });
      comps.push({ kind: "knob", x: x, y: 60, eid: a + "_MATERIAL_PARAM", label: "MATERIAL", lo: "HARD", hi: "SOFT" });
      comps.push({ kind: "trim", x: x - 8.5, y: 73, eid: a + "_DECAYCV_PARAM", label: "DEC" });
      comps.push({ kind: "trim", x: x + 8.5, y: 73, eid: a + "_CTRLCV_PARAM", label: "CTL" });
      comps.push({ kind: "light", x: x, y: 82, eid: a + "_OPEN_LIGHT" });
      comps.push({ kind: "in", x: x - 9, y: 93, eid: a + "_IN_INPUT", label: "IN" });
      comps.push({ kind: "in", x: x + 9, y: 93, eid: a + "_HIT_INPUT", label: "HIT" });
      comps.push({ kind: "in", x: x - 9, y: 105, eid: a + "_DECAY_INPUT", label: "DEC" });
      comps.push({ kind: "in", x: x + 9, y: 105, eid: a + "_CTRL_INPUT", label: "CTL" });
      comps.push({ kind: "out", x: x, y: 117, eid: a + "_OUT_OUTPUT", label: "OUT" });
    });
    notes.push({ x: 40.64, y: 110.0, text: "IMPERF", size: 1.5, col: "eblue" });
    comps.push({ kind: "switch", x: 40.64, y: 117, eid: "IMPERFECTION_PARAM" });
    return { module: "Strike", title: "STRIKE", hp: 16, serial: "002", sub: "DUAL LOW-PASS GATE | ZERO BLEED", comps: comps, notes: notes, dividers: [] };
  }

  function lpgSpec() {
    var x = 15.24, comps = [], notes = [];
    comps.push({ kind: "knob", x: x, y: 20, eid: "RESONANCE_PARAM", label: "RESO" });
    comps.push({ kind: "knob", x: x, y: 40, eid: "DRIVE_PARAM", label: "DRIVE" });
    comps.push({ kind: "switch", x: x, y: 57, eid: "MODE_PARAM" }); notes.push({ x: x, y: 51.0, text: "MODE", size: 1.5, col: "gray" });
    comps.push({ kind: "switch", x: x, y: 71, eid: "OVERSAMPLE_PARAM" }); notes.push({ x: x, y: 65.0, text: "OS", size: 1.5, col: "gray" });
    comps.push({ kind: "in", x: x, y: 92, eid: "AUDIO_INPUT", label: "IN" });
    comps.push({ kind: "in", x: x, y: 104, eid: "CV_INPUT", label: "CV" });
    comps.push({ kind: "out", x: x, y: 117, eid: "AUDIO_OUTPUT", label: "OUT" });
    return { module: "VactrolLPG", title: "LPG", hp: 6, serial: "001", sub: "VACTROL 292 | SINGLE VOICE", comps: comps, notes: notes, dividers: [82.0] };
  }

  root.SAMPanel = {
    PALETTE: C, STYLES: STYLES, THEMES: THEMES, build: build, buildOverlay: buildOverlay,
    SPECS: { Strike: strikeSpec(), VactrolLPG: lpgSpec() }
  };
})(window);
