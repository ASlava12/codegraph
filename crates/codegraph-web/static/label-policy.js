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
    if (labelMode === "focus") return focused && zoom >= 2.1;
    if (focused) return zoom >= 2.35;

    if (hasSearch) {
      if (visibleCount <= 20) return zoom >= 2.35 && priority <= 3;
      if (visibleCount <= 80) return zoom >= 2.9 && priority <= 2;
      return zoom >= 3.25 && priority <= 1;
    }
    if (zoom < 2.85) return false;
    if (visibleCount > 120) return zoom >= 3.35 && priority <= 1;
    if (visibleCount > 50) return zoom >= 3.2 && priority <= 1;
    if (visibleCount > 20) return zoom >= 3.05 && priority <= 2;
    return priority <= 3;
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
      if (zoom < 2.1) return 0;
      return visibleCount <= 40 ? 1 : 0;
    }
    if (zoom < 2.85 && !hasSearch) return 0;
    let budget = visibleCount <= 25 ? 2 : visibleCount <= 50 ? 1 : 0;
    if (zoom >= 3.3 && visibleCount <= 25) budget += 1;
    if (hasSearch && visibleCount <= 30) budget += 1;
    return Math.max(0, Math.min(3, budget));
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
