(function (root, factory) {
  const policy = factory();
  if (typeof module === "object" && module.exports) {
    module.exports = policy;
  }
  root.CodeGraphLabelPolicy = policy;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  function nodeLabelPriority(node) {
    if (node?.metadata?.item_kind === "diagnostic") return 1;
    switch (node?.kind) {
      case "entrypoint":
        return 1;
      case "repository":
        return 2;
      case "config":
      case "environment":
        return 3;
      case "directory":
      case "file":
      case "module":
      case "type":
        return 5;
      case "function":
        return 7;
      case "external_dependency":
        return 9;
      default:
        return 8;
    }
  }

  function shouldShowNodeLabel(options) {
    const {
      selected = false,
      hovered = false,
      focused = false,
      labelMode = "minimal",
      zoom = 1,
      visibleCount = 0,
      hasSearch = false,
      priority = 8,
    } = options || {};

    if (labelMode === "minimal") return false;
    if (labelMode === "hover") return selected || hovered;
    if (selected) return zoom >= 2.6 && visibleCount <= 12;
    if (hovered) return zoom >= 3.6 && visibleCount <= 8;
    if (labelMode === "focus") return focused && zoom >= 3.8 && visibleCount <= 6;
    if (focused) return zoom >= 4.2 && visibleCount <= 5;

    if (hasSearch) {
      if (visibleCount <= 6) return zoom >= 4.4 && priority <= 1;
      if (visibleCount <= 10) return zoom >= 5 && priority <= 1;
      return false;
    }
    if (zoom < 5) return false;
    if (visibleCount > 6) return false;
    if (visibleCount > 4) return priority <= 1;
    return priority <= 1;
  }

  function nodeLabelBudget(options) {
    const {
      labelMode = "minimal",
      zoom = 1,
      visibleCount = 0,
      hasSearch = false,
    } = options || {};

    if (labelMode === "minimal") return 0;
    if (labelMode === "hover") return 0;
    if (labelMode === "focus") {
      if (zoom < 3.8) return 0;
      return visibleCount <= 6 ? 1 : 0;
    }
    if (hasSearch) {
      if (zoom < 4.4) return 0;
      return visibleCount <= 6 ? 1 : visibleCount <= 10 && zoom >= 5 ? 1 : 0;
    }
    if (zoom < 5) return 0;
    if (visibleCount > 6) return 0;
    return 1;
  }

  function truncateGraphLabel(value, maxLength) {
    const label = String(value || "");
    if (label.length <= maxLength) return label;
    return `${label.slice(0, Math.max(0, maxLength - 3))}...`;
  }

  function compactGraphLabelLines(value, maxLength = 24, maxLines = 2) {
    const label = String(value || "").trim();
    if (!label) return [""];
    const lineLength = Math.max(4, Number(maxLength) || 24);
    const lineCount = Math.max(1, Number(maxLines) || 2);
    if (label.length <= lineLength) return [label];

    const lines = [];
    let remaining = label;
    while (remaining && lines.length < lineCount) {
      const lastLine = lines.length === lineCount - 1;
      if (remaining.length <= lineLength) {
        lines.push(remaining);
        break;
      }
      if (lastLine) {
        lines.push(truncateGraphLabel(remaining, lineLength));
        break;
      }

      const breakIndex = preferredLabelBreakIndex(remaining, lineLength);
      const splitAt = breakIndex > 0 ? breakIndex : lineLength;
      const chunk = remaining
        .slice(0, splitAt)
        .replace(/[\\/._:\-\s]+$/g, "");
      lines.push(chunk || remaining.slice(0, lineLength));
      remaining = remaining.slice(splitAt).replace(/^[\\/._:\-\s]+/g, "");
    }

    return lines.length ? lines : [truncateGraphLabel(label, lineLength)];
  }

  function preferredLabelBreakIndex(value, maxLength) {
    const segment = value.slice(0, maxLength + 1);
    let best = -1;
    ["/", "\\", ".", "_", "-", ":", " "].forEach((boundary) => {
      best = Math.max(best, segment.lastIndexOf(boundary));
    });
    return best >= Math.floor(maxLength * 0.45) ? best + 1 : -1;
  }

  return {
    compactGraphLabelLines,
    nodeLabelBudget,
    nodeLabelPriority,
    shouldShowNodeLabel,
    truncateGraphLabel,
  };
});
