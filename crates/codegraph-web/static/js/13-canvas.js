// 13-canvas.js — graph canvas layout, drawing, and label placement.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function fitVisibleGraph() {
  if (state.visibleNodes.length === 0) return;

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    if (!position) return;
    const radius = nodeRadius(node) + 24;
    minX = Math.min(minX, position.x - radius);
    minY = Math.min(minY, position.y - radius);
    maxX = Math.max(maxX, position.x + radius);
    maxY = Math.max(maxY, position.y + radius);
  });

  if (!Number.isFinite(minX) || !Number.isFinite(minY)) return;

  const width = Math.max(1, maxX - minX);
  const height = Math.max(1, maxY - minY);
  const padding = 72;
  const zoomX = (cssWidth(canvas) - padding * 2) / width;
  const zoomY = (cssHeight(canvas) - padding * 2) / height;
  state.zoom = Math.max(0.18, Math.min(3.5, Math.min(zoomX, zoomY)));
  state.pan = {
    x: cssWidth(canvas) / 2 - ((minX + maxX) / 2) * state.zoom,
    y: cssHeight(canvas) / 2 - ((minY + maxY) / 2) * state.zoom,
  };
  renderGraphHud();
  draw();
}

function resetGraphLayout() {
  if (state.graph.nodes.length === 0) return;
  state.positions.clear();
  state.velocities.clear();
  seedGraphLayout();
  state.pan = { x: cssWidth(canvas) / 2, y: cssHeight(canvas) / 2 };
  state.zoom = 1;
  state.layoutPaused = false;
  renderViewportControls();
  draw();
}

function toggleLayout() {
  if (state.graph.nodes.length === 0) return;
  state.layoutPaused = !state.layoutPaused;
  renderViewportControls();
  draw();
}

function panGraphBy(dx, dy) {
  state.pan.x += dx;
  state.pan.y += dy;
  draw();
}

function simulateLayout() {
  const nodes = state.visibleNodes;
  const edges = state.visibleEdges;
  if (nodes.length === 0) return;

  const visibleIds = new Set(nodes.map((node) => node.id));
  const centerPull = 0.004;
  const linkDistance = 112;
  const linkStrength = 0.012;
  const charge = 2800;

  for (let i = 0; i < nodes.length; i += 1) {
    const a = nodes[i];
    const pa = state.positions.get(a.id);
    const va = state.velocities.get(a.id);

    for (let j = i + 1; j < nodes.length; j += 1) {
      const b = nodes[j];
      const pb = state.positions.get(b.id);
      const vb = state.velocities.get(b.id);
      let dx = pa.x - pb.x;
      let dy = pa.y - pb.y;
      let distanceSq = dx * dx + dy * dy + 0.01;
      const distance = Math.sqrt(distanceSq);
      dx /= distance;
      dy /= distance;
      const force = Math.min(6, charge / distanceSq);
      va.x += dx * force;
      va.y += dy * force;
      vb.x -= dx * force;
      vb.y -= dy * force;
    }
  }

  edges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    const sourceVelocity = state.velocities.get(edge.source);
    const targetVelocity = state.velocities.get(edge.target);
    const dx = target.x - source.x;
    const dy = target.y - source.y;
    const distance = Math.max(1, Math.sqrt(dx * dx + dy * dy));
    const force = (distance - linkDistance) * linkStrength;
    const fx = (dx / distance) * force;
    const fy = (dy / distance) * force;
    sourceVelocity.x += fx;
    sourceVelocity.y += fy;
    targetVelocity.x -= fx;
    targetVelocity.y -= fy;
  });

  nodes.forEach((node) => {
    if (node.id === state.draggingId) return;
    const position = state.positions.get(node.id);
    const velocity = state.velocities.get(node.id);
    velocity.x += -position.x * centerPull;
    velocity.y += -position.y * centerPull;
    velocity.x *= 0.82;
    velocity.y *= 0.82;
    position.x += velocity.x;
    position.y += velocity.y;
  });
}

function draw() {
  ctx.clearRect(0, 0, cssWidth(canvas), cssHeight(canvas));
  ctx.save();
  ctx.translate(state.pan.x, state.pan.y);
  ctx.scale(state.zoom, state.zoom);

  const visibleIds = new Set(state.visibleNodes.map((node) => node.id));
  const neighborhood = graphNeighborhoodContext(visibleIds);
  const highlightedEdges = [];
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const emphasis = edgeEmphasis(edge);
    if (emphasis !== "normal") {
      highlightedEdges.push([edge, emphasis]);
      return;
    }
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    const alpha = edgeNeighborhoodAlpha(edge, neighborhood);
    if (alpha < 1) ctx.globalAlpha = alpha;
    drawEdge(edge, source, target, "normal");
    if (alpha < 1) ctx.globalAlpha = 1;
  });

  highlightedEdges.forEach(([edge, emphasis]) => {
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    drawEdge(edge, source, target, emphasis);
  });

  const labelCandidates = [];
  const riskByNode = state.riskByNode;
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    const selected = node.id === state.selectedId;
    const hovered = node.id === state.hoveredId;
    const focused = nodeIsFocused(node);
    const neighbor = nodeIsNeighborhoodNeighbor(node, neighborhood);
    const muted = nodeIsNeighborhoodMuted(node, neighborhood, selected, hovered, focused);
    const radius = nodeRadius(node);

    if (muted) ctx.globalAlpha = 0.34;
    ctx.beginPath();
    ctx.arc(
      position.x,
      position.y,
      radius + (selected ? 6 : focused ? 5 : hovered ? 3 : neighbor ? 4 : 0),
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = selected
      ? "rgba(92, 200, 167, 0.26)"
      : focused
        ? "rgba(237, 241, 242, 0.16)"
        : hovered
          ? "rgba(255,255,255,0.12)"
          : neighbor
            ? "rgba(92, 200, 167, 0.13)"
          : "rgba(0,0,0,0.22)";
    ctx.fill();

    ctx.beginPath();
    ctx.arc(position.x, position.y, radius, 0, Math.PI * 2);
    ctx.fillStyle = colorFor(node.kind);
    ctx.fill();
    ctx.lineWidth = selected ? 2.6 / state.zoom : focused ? 2.2 / state.zoom : neighbor ? 1.8 / state.zoom : 1 / state.zoom;
    ctx.strokeStyle = selected
      ? "#ffffff"
      : focused
        ? "rgba(237, 241, 242, 0.92)"
        : neighbor
          ? "rgba(92, 200, 167, 0.84)"
          : "rgba(255,255,255,0.55)";
    ctx.stroke();

    const riskSeverity = riskByNode.get(Number(node.id));
    if (riskSeverity) {
      drawRiskHalo(position, radius, riskSeverity, selected || focused || hovered || neighbor);
    }
    if (muted) ctx.globalAlpha = 1;

    if (!muted && shouldShowNodeLabel(node, selected, hovered, focused)) {
      labelCandidates.push({
        node,
        position,
        radius,
        selected,
        hovered,
        focused,
        forced: false,
        priority: nodeLabelPriority(node),
        bypassBudget: state.labelMode === "hover" && hovered,
      });
    }
  });

  drawNodeLabels(labelCandidates);
  ctx.restore();
  drawGraphMinimap();
}

function graphWorldBounds(padding = 32) {
  if (state.visibleNodes.length === 0) return null;

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    if (!position) return;
    const radius = nodeRadius(node);
    minX = Math.min(minX, position.x - radius);
    minY = Math.min(minY, position.y - radius);
    maxX = Math.max(maxX, position.x + radius);
    maxY = Math.max(maxY, position.y + radius);
  });

  if (!Number.isFinite(minX) || !Number.isFinite(minY)) return null;

  if (maxX - minX < 1) {
    minX -= 40;
    maxX += 40;
  }
  if (maxY - minY < 1) {
    minY -= 40;
    maxY += 40;
  }

  return {
    minX: minX - padding,
    minY: minY - padding,
    maxX: maxX + padding,
    maxY: maxY + padding,
  };
}

function minimapTransform() {
  const rect = minimapCanvas.getBoundingClientRect();
  const width = Math.max(1, Math.floor(rect.width));
  const height = Math.max(1, Math.floor(rect.height));
  const bounds = graphWorldBounds(48);
  if (!bounds) return null;

  // Backing store in device pixels, drawing in CSS pixels (see resizeCanvas).
  const ratio = state.pixelRatio || 1;
  const backingWidth = Math.floor(width * ratio);
  const backingHeight = Math.floor(height * ratio);
  if (minimapCanvas.width !== backingWidth || minimapCanvas.height !== backingHeight) {
    minimapCanvas.width = backingWidth;
    minimapCanvas.height = backingHeight;
    minimapCtx.setTransform(ratio, 0, 0, ratio, 0, 0);
  }

  const padding = 10;
  const graphWidth = Math.max(1, bounds.maxX - bounds.minX);
  const graphHeight = Math.max(1, bounds.maxY - bounds.minY);
  const scale = Math.min(
    Math.max(1, width - padding * 2) / graphWidth,
    Math.max(1, height - padding * 2) / graphHeight,
  );
  const offsetX = (width - graphWidth * scale) / 2 - bounds.minX * scale;
  const offsetY = (height - graphHeight * scale) / 2 - bounds.minY * scale;
  return { bounds, width, height, scale, offsetX, offsetY };
}

function worldToMinimap(point, transform) {
  return {
    x: point.x * transform.scale + transform.offsetX,
    y: point.y * transform.scale + transform.offsetY,
  };
}

function minimapToWorld(point, transform) {
  return {
    x: (point.x - transform.offsetX) / transform.scale,
    y: (point.y - transform.offsetY) / transform.scale,
  };
}

function drawGraphMinimap() {
  if (state.visibleNodes.length === 0) {
    minimapCanvas.hidden = true;
    return;
  }
  minimapCanvas.hidden = false;

  const transform = minimapTransform();
  if (!transform) return;

  minimapCtx.clearRect(0, 0, transform.width, transform.height);
  minimapCtx.fillStyle = "rgba(16, 18, 20, 0.82)";
  minimapCtx.fillRect(0, 0, transform.width, transform.height);

  const visibleIds = new Set(state.visibleNodes.map((node) => node.id));
  minimapCtx.lineWidth = 1;
  minimapCtx.strokeStyle = "rgba(237, 241, 242, 0.16)";
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    if (!source || !target) return;
    const start = worldToMinimap(source, transform);
    const end = worldToMinimap(target, transform);
    minimapCtx.beginPath();
    minimapCtx.moveTo(start.x, start.y);
    minimapCtx.lineTo(end.x, end.y);
    minimapCtx.stroke();
  });

  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    if (!position) return;
    const point = worldToMinimap(position, transform);
    const selected = node.id === state.selectedId;
    const focused = nodeIsFocused(node);
    minimapCtx.beginPath();
    minimapCtx.arc(point.x, point.y, selected || focused ? 3.5 : 2.4, 0, Math.PI * 2);
    minimapCtx.fillStyle = selected ? "#ffffff" : focused ? "#5cc8a7" : colorFor(node.kind);
    minimapCtx.fill();
  });

  const topLeft = worldToMinimap(screenToWorld(0, 0), transform);
  const bottomRight = worldToMinimap(screenToWorld(cssWidth(canvas), cssHeight(canvas)), transform);
  const viewX = Math.min(topLeft.x, bottomRight.x);
  const viewY = Math.min(topLeft.y, bottomRight.y);
  const viewWidth = Math.abs(bottomRight.x - topLeft.x);
  const viewHeight = Math.abs(bottomRight.y - topLeft.y);
  minimapCtx.fillStyle = "rgba(92, 200, 167, 0.12)";
  minimapCtx.strokeStyle = "rgba(92, 200, 167, 0.92)";
  minimapCtx.lineWidth = 1.4;
  minimapCtx.fillRect(viewX, viewY, viewWidth, viewHeight);
  minimapCtx.strokeRect(viewX, viewY, viewWidth, viewHeight);
}

function drawRiskHalo(position, radius, severity, emphasized) {
  const zoom = Math.max(0.18, state.zoom);
  ctx.beginPath();
  ctx.arc(position.x, position.y, radius + (emphasized ? 8 : 5) / zoom, 0, Math.PI * 2);
  ctx.lineWidth = (emphasized ? 3.2 : 2.2) / zoom;
  ctx.strokeStyle = riskColor(severity);
  ctx.stroke();
}

function drawEdge(edge, source, target, emphasis) {
  if (!source || !target) return;
  const emphasized = emphasis !== "normal";
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const distance = Math.max(1, Math.sqrt(dx * dx + dy * dy));
  const ux = dx / distance;
  const uy = dy / distance;
  const sourceRadius = nodeRadiusById(edge.source) + 2 / state.zoom;
  const targetRadius = nodeRadiusById(edge.target) + (emphasized ? 8 : 3) / state.zoom;
  const start = {
    x: source.x + ux * Math.min(sourceRadius, distance * 0.35),
    y: source.y + uy * Math.min(sourceRadius, distance * 0.35),
  };
  const end = {
    x: target.x - ux * Math.min(targetRadius, distance * 0.35),
    y: target.y - uy * Math.min(targetRadius, distance * 0.35),
  };

  if (emphasized) {
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    ctx.lineTo(end.x, end.y);
    ctx.lineWidth = edgeBackplateWidth(emphasis) / state.zoom;
    ctx.strokeStyle = "rgba(13, 15, 16, 0.76)";
    ctx.stroke();
  }

  ctx.beginPath();
  ctx.moveTo(start.x, start.y);
  ctx.lineTo(end.x, end.y);
  ctx.lineWidth = edgeStrokeWidth(emphasis) / state.zoom;
  ctx.strokeStyle = emphasis === "normal" ? edgeColor(edge) : edgeHighlightColor(emphasis);
  ctx.stroke();

  if (emphasized) {
    drawArrowHead(start, end, edgeHighlightColor(emphasis));
  }
}

function drawArrowHead(start, end, color) {
  const angle = Math.atan2(end.y - start.y, end.x - start.x);
  const length = 11 / state.zoom;
  const spread = Math.PI / 7;
  ctx.beginPath();
  ctx.moveTo(end.x, end.y);
  ctx.lineTo(end.x - Math.cos(angle - spread) * length, end.y - Math.sin(angle - spread) * length);
  ctx.lineTo(end.x - Math.cos(angle + spread) * length, end.y - Math.sin(angle + spread) * length);
  ctx.closePath();
  ctx.fillStyle = color;
  ctx.fill();
}

function shouldShowNodeLabel(node, selected, hovered, focused) {
  return CodeGraphLabelPolicy.shouldShowNodeLabel({
    selected,
    hovered,
    focused,
    labelMode: state.labelMode,
    zoom: state.zoom,
    visibleCount: state.visibleNodes.length,
    hasSearch: Boolean(state.search),
    priority: nodeLabelPriority(node),
  });
}

function drawNodeLabels(candidates) {
  const occupied = [];
  const nodeBoxes = nodeOcclusionBoxes();
  const edgeBoxes = edgeOcclusionBoxes();
  const budget = nodeLabelBudget();
  let drawnAutoLabels = 0;
  const ordered = candidates.sort((left, right) => {
    const leftPriority = left.selected ? 0 : left.hovered ? 1 : left.focused ? 2 : 3;
    const rightPriority = right.selected ? 0 : right.hovered ? 1 : right.focused ? 2 : 3;
    return (
      leftPriority - rightPriority ||
      left.priority - right.priority ||
      left.node.label.localeCompare(right.node.label)
    );
  });

  ordered.forEach((candidate) => {
    if (!candidate.bypassBudget && drawnAutoLabels >= budget) return;
    const geometry = labelGeometry(candidate, occupied, nodeBoxes, edgeBoxes);
    if (!geometry) return;
    drawLabelGeometry(geometry);
    occupied.push(geometry);
    if (!candidate.bypassBudget) drawnAutoLabels += 1;
  });
}

function labelGeometry(candidate, occupied, nodeBoxes, edgeBoxes) {
  const { node, position, radius, forced, hovered, focused } = candidate;
  const zoom = Math.max(0.18, state.zoom);
  const textLength = hovered ? 12 : focused ? 11 : 8;
  const lines = forced
    ? compactGraphLabelLines(node.label, 14, 1)
    : [truncateGraphLabel(node.label, textLength)];
  const padX = (forced ? 6 : 4) / zoom;
  const padY = (forced ? 4 : 2.5) / zoom;
  const fontSize = (forced ? 11 : 8) / zoom;
  const lineHeight = (forced ? 12 : 9) / zoom;
  ctx.font = `${fontSize}px Inter, sans-serif`;
  const width = Math.max(...lines.map((line) => ctx.measureText(line).width)) + padX * 2;
  const height = lines.length * lineHeight + padY * 2;
  const gap = (forced ? 11 : 21) / zoom;
  const placements = forced ? ["right", "left", "top"] : ["right", "left"];
  const geometries = placements.map((placement) =>
    clampLabelGeometryToViewport(labelGeometryForPlacement({
      node,
      position,
      radius,
      lines,
      width,
      height,
      padX,
      gap,
      lineHeight,
      font: ctx.font,
      forced,
      placement,
    })),
  );
  const usable = geometries.find(
    (geometry) =>
      boxIntersectsViewport(geometry) && !labelIntersectsScene(geometry, occupied, nodeBoxes, edgeBoxes),
  );
  if (usable) return usable;
  return null;
}

function labelGeometryForPlacement(options) {
  const {
    node,
    position,
    radius,
    lines,
    width,
    height,
    padX,
    gap,
    lineHeight,
    font,
    forced,
    placement,
  } = options;
  let x = position.x - width / 2;
  let y = position.y + radius + gap;

  if (placement === "top") {
    y = position.y - radius - gap - height;
  } else if (placement === "right") {
    x = position.x + radius + gap;
    y = position.y - height / 2;
  } else if (placement === "left") {
    x = position.x - radius - gap - width;
    y = position.y - height / 2;
  }

  return {
    nodeId: node.id,
    lines,
    x,
    y,
    width,
    height,
    padX,
    textY: y + height / 2,
    lineHeight,
    radius: 5 / Math.max(0.18, state.zoom),
    font,
    forced,
  };
}

function drawLabelGeometry(geometry) {
  ctx.font = geometry.font;
  ctx.textBaseline = "middle";
  if (!geometry.forced) {
    ctx.fillStyle = "rgba(13, 15, 16, 0.78)";
    roundRect(ctx, geometry.x, geometry.y, geometry.width, geometry.height, geometry.radius);
    ctx.fill();
    ctx.lineWidth = 1 / Math.max(0.18, state.zoom);
    ctx.strokeStyle = "rgba(237, 241, 242, 0.16)";
    ctx.stroke();
    ctx.fillStyle = "rgba(237, 241, 242, 0.9)";
    drawLabelText(geometry, (line, x, y) => ctx.fillText(line, x, y));
    return;
  }
  ctx.fillStyle = geometry.forced
    ? "rgba(13, 15, 16, 0.84)"
    : "rgba(13, 15, 16, 0.58)";
  roundRect(ctx, geometry.x, geometry.y, geometry.width, geometry.height, geometry.radius);
  ctx.fill();
  if (geometry.forced) {
    ctx.lineWidth = 1 / Math.max(0.18, state.zoom);
    ctx.strokeStyle = "rgba(237, 241, 242, 0.22)";
    ctx.stroke();
  }
  ctx.fillStyle = "#edf1f2";
  drawLabelText(geometry, (line, x, y) => ctx.fillText(line, x, y));
}

function clampLabelGeometryToViewport(geometry) {
  const zoom = Math.max(0.18, state.zoom);
  const margin = 8 / zoom;
  const minX = (-state.pan.x / zoom) + margin;
  const minY = (-state.pan.y / zoom) + margin;
  const maxX = ((cssWidth(canvas) - state.pan.x) / zoom) - geometry.width - margin;
  const maxY = ((cssHeight(canvas) - state.pan.y) / zoom) - geometry.height - margin;
  if (maxX >= minX) geometry.x = Math.max(minX, Math.min(maxX, geometry.x));
  if (maxY >= minY) geometry.y = Math.max(minY, Math.min(maxY, geometry.y));
  geometry.textY = geometry.y + geometry.height / 2;
  return geometry;
}

function drawLabelText(geometry, drawLine) {
  const firstY = geometry.textY - ((geometry.lines.length - 1) * geometry.lineHeight) / 2;
  geometry.lines.forEach((line, index) => {
    drawLine(line, geometry.x + geometry.padX, firstY + index * geometry.lineHeight);
  });
}

function nodeLabelPriority(node) {
  return CodeGraphLabelPolicy.nodeLabelPriority(node);
}

function nodeLabelBudget() {
  return CodeGraphLabelPolicy.nodeLabelBudget({
    labelMode: state.labelMode,
    zoom: state.zoom,
    visibleCount: state.visibleNodes.length,
    hasSearch: Boolean(state.search),
  });
}

function nodeOcclusionBoxes() {
  const pad = 32 / Math.max(0.18, state.zoom);
  return state.visibleNodes
    .map((node) => {
      const position = state.positions.get(node.id);
      if (!position) return null;
      const radius = nodeRadius(node) + pad;
      return {
        nodeId: node.id,
        x: position.x - radius,
        y: position.y - radius,
        width: radius * 2,
        height: radius * 2,
      };
    })
    .filter(Boolean);
}

function edgeOcclusionBoxes() {
  const pad = 8 / Math.max(0.18, state.zoom);
  return state.visibleEdges
    .map((edge) => {
      const source = state.positions.get(edge.source);
      const target = state.positions.get(edge.target);
      if (!source || !target) return null;
      const minX = Math.min(source.x, target.x) - pad;
      const minY = Math.min(source.y, target.y) - pad;
      return {
        source: edge.source,
        target: edge.target,
        x: minX,
        y: minY,
        width: Math.abs(source.x - target.x) + pad * 2,
        height: Math.abs(source.y - target.y) + pad * 2,
      };
    })
    .filter(Boolean);
}

function truncateGraphLabel(value, maxLength) {
  return CodeGraphLabelPolicy.truncateGraphLabel(value, maxLength);
}

function compactGraphLabelLines(value, maxLength, maxLines) {
  return CodeGraphLabelPolicy.compactGraphLabelLines(value, maxLength, maxLines);
}

function boxIntersectsViewport(box) {
  const left = box.x * state.zoom + state.pan.x;
  const right = (box.x + box.width) * state.zoom + state.pan.x;
  const top = box.y * state.zoom + state.pan.y;
  const bottom = (box.y + box.height) * state.zoom + state.pan.y;
  const margin = 24;
  return !(right < -margin || left > cssWidth(canvas) + margin || bottom < -margin || top > cssHeight(canvas) + margin);
}

function labelIntersectsScene(label, occupied, nodeBoxes, edgeBoxes) {
  return (
    occupied.some((box) => boxesIntersect(box, label)) ||
    nodeBoxes.some((box) => box.nodeId !== label.nodeId && boxesIntersect(box, label)) ||
    edgeBoxes.some(
      (box) =>
        box.source !== label.nodeId &&
        box.target !== label.nodeId &&
        boxesIntersect(box, label),
    )
  );
}

function boxesIntersect(left, right) {
  const pad = 12 / Math.max(0.18, state.zoom);
  return !(
    left.x + left.width + pad < right.x ||
    right.x + right.width + pad < left.x ||
    left.y + left.height + pad < right.y ||
    right.y + right.height + pad < left.y
  );
}

function renderSelection() {
  state.selectionRequest += 1;
  const requestId = state.selectionRequest;
  if (state.selectedEdgeKey) {
    const edgeRecord = selectedEdge();
    if (edgeRecord) {
      renderEdgeSelectionPanel(edgeRecord.edge, edgeRecord.source, edgeRecord.target);
    } else {
      clearLastSelectionCard();
      selectionTitle.textContent = t("selection.edge");
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("selection.noEdge"))}</p>`;
    }
    return;
  }
  const node = state.graph.nodes.find((candidate) => candidate.id === state.selectedId);
  if (!node) {
    clearLastSelectionCard();
    if (state.selectedId != null) {
      selectionTitle.textContent = `${t("selection.node")} ${state.selectedId}`;
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("selection.loading"))}</p>`;
      loadNodeContext(state.selectedId, requestId);
    } else {
      selectionTitle.textContent = t("selection.title");
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("selection.noNode"))}</p>`;
    }
    return;
  }

  renderSelectionPanel(node, [], new Map([[node.id, node]]), requestId, true);
  loadNodeContext(node.id, requestId);
}

function selectedEdge() {
  const key = state.selectedEdgeKey;
  if (!key) return null;
  const edge =
    state.edgeSelectionCache.get(key) ||
    state.visibleEdges.find((edge) => edgeSelectionKey(edge) === key) ||
    state.graph.edges.find((edge) => edgeSelectionKey(edge) === key) ||
    null;
  if (!edge) return null;
  const cachedNodes = state.edgeSelectionNodeCache.get(key) || {};
  return {
    edge,
    source: cachedNodes.source || graphNodeById(edge.source),
    target: cachedNodes.target || graphNodeById(edge.target),
  };
}

function renderEdgeSelectionPanel(edge, source = null, target = null) {
  const edgeIndex = edgeIndexOf(edge);
  const metadataRows = Object.entries(edge.metadata || {})
    .filter(([key]) => key !== "edge_index")
    .map(([key, value]) => [formatKind(key), value]);

  setLastSelectionCard(buildEdgeSelectionCard(edge, source, target));
  selectionTitle.textContent = t("selection.edge");
  selectionBody.innerHTML = `
    <div class="node-card edge-card">
      <header class="node-card-header">
        <div class="node-card-title">
          <span>${escapeHtml(formatKind(edge.kind))}</span>
          <strong>${escapeHtml(source?.label || String(edge.source))}</strong>
          <em>${escapeHtml(target?.label || String(edge.target))}</em>
        </div>
        ${edgeIndex == null ? "" : `<span class="node-card-id">#${edgeIndex}</span>`}
      </header>
      <div class="selection-actions">
        <button type="button" data-node-id="${edge.source}">${escapeHtml(t("selection.openSource"))}</button>
        <button type="button" data-node-id="${edge.target}">${escapeHtml(t("selection.openTarget"))}</button>
        ${
          edgeIndex == null
            ? ""
            : `<button type="button" data-copy-selection-link="edge" data-selection-link-id="${edgeIndex}">${escapeHtml(t("button.copyLink"))}</button>`
        }
        <button type="button" data-export-selection-card>${escapeHtml(t("button.downloadCard"))}</button>
        ${
          edgeIndex == null
            ? ""
            : `<button type="button" data-focus-edge-index="${edgeIndex}">${escapeHtml(t("button.focusEdge"))}</button>
               <button type="button" data-query-edge-index="${edgeIndex}">${escapeHtml(t("button.queryEdge"))}</button>`
        }
      </div>
      <section class="node-card-section">
        <h3>${escapeHtml(t("selection.summary"))}</h3>
        <dl class="node-summary">
          ${[
            [t("selection.edgeIndex"), edgeIndex == null ? "" : String(edgeIndex)],
            [t("selection.kind"), formatKind(edge.kind)],
            [t("label.confidence"), formatKind(edge.confidence || "unknown")],
            [t("label.from"), source?.label || String(edge.source)],
            [t("label.to"), target?.label || String(edge.target)],
          ]
            .filter(([, value]) => value)
            .map(renderDefinitionRow)
            .join("")}
        </dl>
      </section>
      ${
        metadataRows.length > 0
          ? `<section class="node-card-section">
              <h3>${escapeHtml(t("selection.metadata"))}</h3>
              <dl class="node-summary">
                ${metadataRows.map(renderDefinitionRow).join("")}
              </dl>
            </section>`
          : ""
      }
      <section class="node-card-section">
        <div class="edge-row">
          ${renderExplainEdgeButton(edge)}
        </div>
        <div class="edge-explanation" data-edge-explanation hidden></div>
      </section>
    </div>
  `;
  attachQueryNavigation(selectionBody);
  attachEdgeExplainActions(selectionBody);
  attachCopyLinkActions(selectionBody);
  attachSelectionCardExportAction();
  selectionBody.querySelectorAll("[data-focus-edge-index]").forEach((button) => {
    button.addEventListener("click", () => focusEdgeIndex(Number(button.dataset.focusEdgeIndex)));
  });
  selectionBody.querySelectorAll("[data-query-edge-index]").forEach((button) => {
    button.addEventListener("click", () => runEdgeIndexQuery(Number(button.dataset.queryEdgeIndex)));
  });
}

async function loadNodeContext(nodeId, requestId) {
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(nodeId),
    edge_limit: "80",
    source_context: "5",
    insight_limit: "8",
  });

  try {
    const response = await apiFetch(`/api/node-card?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest || state.selectedId !== nodeId) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "node card failed"));
    }
    const context = body.context || {};
    const nodeMap = new Map((context.nodes || []).map((node) => [node.id, node]));
    nodeMap.set(context.node.id, context.node);
    renderSelectionPanel(context.node, context.edges || [], nodeMap, requestId, false, context, body);
  } catch (error) {
    if (requestId !== state.selectionRequest || state.selectedId !== nodeId) return;
    const node = state.graph.nodes.find((candidate) => candidate.id === nodeId);
    if (node) {
      renderSelectionPanel(node, [], new Map([[node.id, node]]), requestId, false);
      const container = selectionBody.querySelector(".neighbors");
      if (container) {
        container.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
      }
    } else {
      selectionTitle.textContent = t("status.error");
      selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    }
  }
}

function renderSelectionPanel(node, edges, nodeMap, requestId, loading = false, context = null, card = null) {
  state.lastWorkflowReport = null;
  if (loading) {
    clearLastSelectionCard();
  } else {
    setLastSelectionCard(buildNodeSelectionCard(node, edges, context, card));
  }
  selectionTitle.textContent = node.label;
  const summaryRows = renderNodeSummaryRows(node);
  const metadataRows = renderNodeMetadataRows(node);
  const nodeIssues = (card?.insights || nodeInsightsForNode(node.id)).slice(0, 8);
  const sourceLines = card?.source?.lines || null;
  const sourcePath = card?.source?.path || node.span?.path || "";
  const sourceLineSuffix = node.span ? `:${node.span.start_line}` : "";
  const cardActions = nodeCardActions(card, node);
  const fileSummary = renderFileSummary(card?.file_summary);
  const dependencySummary = renderDependencySummary(card?.dependency_summary);
  const riskSummary = renderNodeRiskSummary(card?.insight_summary);
  const neighborRows = loading
    ? `<p class="empty">${escapeHtml(t("selection.loading"))}</p>`
    : edges.length > 0
      ? edges.map((edge) => renderNeighbor(edge, node.id, nodeMap)).join("")
      : `<p class="empty">${escapeHtml(t("selection.noDependencies"))}</p>`;
  const contextSummary = context
    ? `<span class="neighbor-summary">${
        escapeHtml(
          context.truncated_edges
            ? t("selection.contextEdgesLimited", {
                count: context.total_edges,
                limit: context.edge_limit,
              })
            : t("selection.contextEdges", { count: context.total_edges }),
        )
      }</span>`
    : "";

  selectionBody.innerHTML = `
    <div class="node-card">
      <header class="node-card-header">
        <div class="node-card-title">
          <span>${escapeHtml(formatKind(node.kind))}</span>
          <strong>${escapeHtml(node.label)}</strong>
        </div>
        <span class="node-card-id">#${node.id}</span>
      </header>
      <div class="selection-actions">
        <button type="button" data-copy-selection-link="node" data-selection-link-id="${node.id}">${escapeHtml(t("button.copyLink"))}</button>
        <button type="button" data-export-selection-card ${loading ? "disabled" : ""}>${escapeHtml(t("button.downloadCard"))}</button>
        <button type="button" data-path-endpoint="from">${escapeHtml(t("selection.from"))}</button>
        <button type="button" data-path-endpoint="to">${escapeHtml(t("selection.to"))}</button>
        ${
          node.kind === "config" || node.kind === "environment"
            ? `<button type="button" data-config-trace-target>${escapeHtml(t("selection.configTrace"))}</button>`
            : ""
        }
        ${
          node.metadata?.item_kind === "error"
            ? `<button type="button" data-error-trace-target>${escapeHtml(t("selection.errorTrace"))}</button>`
            : ""
        }
        ${cardActions
          .map(
            (action, index) =>
              `<button type="button" data-card-query-action="${index}">${escapeHtml(nodeCardActionLabel(action))}</button>`,
          )
          .join("")}
      </div>
      <section class="node-card-section">
        <h3>${escapeHtml(t("selection.summary"))}</h3>
        <dl class="node-summary">
          ${summaryRows.map(renderDefinitionRow).join("")}
        </dl>
        ${fileSummary}
      </section>
      ${
        metadataRows.length > 0
          ? `<section class="node-card-section">
              <h3>${escapeHtml(t("selection.metadata"))}</h3>
              <dl class="node-summary">
                ${metadataRows.map(renderDefinitionRow).join("")}
              </dl>
            </section>`
          : ""
      }
      <section class="node-card-section">
        <div class="node-card-section-header">
          <h3>${escapeHtml(t("selection.dependencies"))}</h3>
          ${contextSummary}
        </div>
        ${dependencySummary}
        <div class="neighbors">${neighborRows}</div>
      </section>
      <section class="node-card-section">
        <div class="node-card-section-header">
          <h3>${escapeHtml(t("selection.risks"))}</h3>
          <span>${Number(card?.total_insights ?? nodeIssues.length)}</span>
        </div>
        ${riskSummary}
        <div class="node-issues">
          ${
            nodeIssues.length > 0
              ? nodeIssues.map(renderNodeIssue).join("")
              : `<p class="empty">${escapeHtml(t("selection.noIssues"))}</p>`
          }
        </div>
      </section>
      <section class="trace-panel">
        <div class="trace-controls">
          <label class="field compact">
            <span>${escapeHtml(t("selection.traceDepth"))}</span>
            <input id="traceDepthInput" type="number" min="1" max="8" value="3" />
          </label>
          <button id="traceButton" type="button">${escapeHtml(t("selection.trace"))}</button>
          <button id="workflowButton" type="button">${escapeHtml(t("selection.flow"))}</button>
          <button id="dependentsButton" type="button">${escapeHtml(t("selection.dependents"))}</button>
        </div>
        <div class="workflow-filter-fields">
          <label class="field compact">
            <span>${escapeHtml(t("label.edge"))}</span>
            <input id="workflowEdgeKindInput" type="text" list="edgeKindOptions" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.confidence"))}</span>
            <input id="workflowConfidenceInput" type="text" list="confidenceOptions" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.language"))}</span>
            <input id="workflowLanguageInput" type="text" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.riskSeverity"))}</span>
            <input id="workflowRiskSeverityInput" type="text" list="severityOptions" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.block"))}</span>
            <input id="workflowBlockKindInput" type="text" list="workflowBlockKindOptions" autocomplete="off" />
          </label>
        </div>
        <div class="workflow-export-actions">
          <button id="workflowJsonExportButton" type="button" disabled>${escapeHtml(t("button.downloadWorkflow"))}</button>
          <button id="workflowMermaidExportButton" type="button" disabled>${escapeHtml(t("button.downloadWorkflowMermaid"))}</button>
          <button id="workflowDotExportButton" type="button" disabled>${escapeHtml(t("button.downloadWorkflowDot"))}</button>
        </div>
        <div id="traceResult" class="trace-result"></div>
      </section>
      ${
        sourceLines || node.span
          ? `<section class="source-preview">
            <header>
              <span>${escapeHtml(t("selection.source"))}</span>
              <strong>${escapeHtml(`${sourcePath}${sourceLineSuffix}`)}</strong>
              ${card?.source?.truncated ? `<span>${escapeHtml(t("selection.sourceTruncated"))}</span>` : ""}
            </header>
            <pre id="sourcePreview"><code>${
              sourceLines ? sourceLines.map(renderSourceLine).join("") : escapeHtml(t("empty.loadingSource"))
            }</code></pre>
          </section>`
          : `<section class="source-preview">
              <header>
                <span>${escapeHtml(t("selection.source"))}</span>
              </header>
              <p class="empty">${escapeHtml(t("selection.noSource"))}</p>
            </section>`
      }
    </div>
  `;

  selectionBody.querySelectorAll(".neighbor").forEach((button) => {
    button.addEventListener("click", () => {
      selectNodeById(button.dataset.nodeId);
    });
  });

  selectionBody.querySelectorAll("[data-path-endpoint]").forEach((button) => {
    button.addEventListener("click", () => {
      const target = button.dataset.pathEndpoint === "to" ? pathToInput : pathFromInput;
      target.value = String(node.id);
      target.focus();
    });
  });

  const configTraceTarget = selectionBody.querySelector("[data-config-trace-target]");
  if (configTraceTarget) {
    configTraceTarget.addEventListener("click", () => {
      configTraceTargetInput.value = node.label;
      runConfigTrace();
    });
  }

  const errorTraceTarget = selectionBody.querySelector("[data-error-trace-target]");
  if (errorTraceTarget) {
    errorTraceTarget.addEventListener("click", () => {
      errorTraceTargetInput.value = node.label;
      runErrorTrace();
    });
  }

  selectionBody.querySelectorAll("[data-card-query-action]").forEach((button) => {
    const action = cardActions[Number(button.dataset.cardQueryAction)];
    if (action?.query) {
      button.addEventListener("click", () => runNodeCardQuery(action.query));
    }
  });

  const traceButton = document.querySelector("#traceButton");
  if (traceButton) {
    traceButton.addEventListener("click", () => loadTrace(node));
  }
  const workflowButton = document.querySelector("#workflowButton");
  if (workflowButton) {
    workflowButton.addEventListener("click", () => loadWorkflow(node));
  }
  for (const input of workflowFilterInputs("workflow")) {
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") loadWorkflow(node);
    });
  }
  renderWorkflowExportState();
  const workflowJsonExportButton = document.querySelector("#workflowJsonExportButton");
  if (workflowJsonExportButton) {
    workflowJsonExportButton.addEventListener("click", () => exportLastWorkflowReport("json"));
  }
  const workflowMermaidExportButton = document.querySelector("#workflowMermaidExportButton");
  if (workflowMermaidExportButton) {
    workflowMermaidExportButton.addEventListener("click", () => exportLastWorkflowReport("mermaid"));
  }
  const workflowDotExportButton = document.querySelector("#workflowDotExportButton");
  if (workflowDotExportButton) {
    workflowDotExportButton.addEventListener("click", () => exportLastWorkflowReport("dot"));
  }
  const dependentsButton = document.querySelector("#dependentsButton");
  if (dependentsButton) {
    dependentsButton.addEventListener("click", () => loadDependents(node));
  }

  selectionBody.querySelectorAll("[data-node-issue-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const issue = nodeIssues[Number(button.dataset.nodeIssueIndex)];
      if (issue) focusInsight(issue);
    });
  });

  if (node.span && !loading && !sourceLines) {
    loadSourcePreview(node, requestId);
  }

  attachEdgeExplainActions(selectionBody);
  attachCopyLinkActions(selectionBody);
  attachSelectionCardExportAction();
}

function renderNodeSummaryRows(node) {
  const rows = [
    [t("selection.kind"), formatKind(node.kind)],
    [t("selection.id"), String(node.id)],
  ];
  if (node.metadata?.language) rows.push([t("label.language"), node.metadata.language]);
  if (node.metadata?.item_kind) rows.push([t("label.item"), formatKind(node.metadata.item_kind)]);
  if (node.kind === "file") rows.push([t("selection.path"), node.label]);
  if (node.span) {
    rows.push([t("selection.path"), node.span.path]);
    rows.push([t("selection.lines"), `${node.span.start_line}-${node.span.end_line}`]);
  }
  return rows;
}
