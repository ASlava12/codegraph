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

    if (selected || hovered) return true;
    if (labelMode === "minimal") return false;
    if (labelMode === "focus") return focused && zoom >= 2.35;
    if (focused) return zoom >= 2.65;

    if (hasSearch) {
      if (visibleCount <= 18) return zoom >= 2.65 && priority <= 2;
      if (visibleCount <= 35) return zoom >= 3.1 && priority <= 1;
      return false;
    }
    if (zoom < 3.25) return false;
    if (visibleCount > 25) return false;
    if (visibleCount > 12) return zoom >= 3.6 && priority <= 1;
    return priority <= 2;
  }

  function nodeLabelBudget(options) {
    const {
      labelMode = "minimal",
      zoom = 1,
      visibleCount = 0,
      hasSearch = false,
    } = options || {};

    if (labelMode === "minimal") return 0;
    if (labelMode === "focus") {
      if (zoom < 2.35) return 0;
      return visibleCount <= 25 ? 1 : 0;
    }
    if (hasSearch) {
      if (zoom < 2.65) return 0;
      return visibleCount <= 18 ? 2 : visibleCount <= 35 ? 1 : 0;
    }
    if (zoom < 3.25) return 0;
    if (visibleCount > 25) return 0;
    if (visibleCount <= 12 && zoom >= 4) return 2;
    return 1;
  }

  function truncateGraphLabel(value, maxLength) {
    const label = String(value || "");
    if (label.length <= maxLength) return label;
    return `${label.slice(0, Math.max(0, maxLength - 3))}...`;
  }

  return {
    nodeLabelBudget,
    nodeLabelPriority,
    shouldShowNodeLabel,
    truncateGraphLabel,
  };
});
