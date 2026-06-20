const TOOLTIP_SELECTOR = "[data-tooltip]";
const VIEWPORT_PADDING = 8;
const MAX_TOOLTIP_WIDTH = 256;
const TOOLTIP_GAP = 9;

let installed = false;
let activeTooltipTarget: HTMLElement | null = null;
let tooltipElement: HTMLDivElement | null = null;
let tooltipTextElement: HTMLDivElement | null = null;
let tooltipArrowElement: HTMLDivElement | null = null;

export function installTooltipOverflowGuard() {
  if (installed) return;
  installed = true;

  document.addEventListener("pointerover", handleTooltipEnter, true);
  document.addEventListener("pointerout", handleTooltipLeave, true);
  document.addEventListener("mouseover", handleTooltipEnter, true);
  document.addEventListener("mouseout", handleTooltipLeave, true);
  document.addEventListener("focusin", handleTooltipEnter, true);
  document.addEventListener("focusout", handleTooltipLeave, true);
  window.addEventListener("resize", updateActiveTooltipAlignment);
  window.addEventListener("scroll", updateActiveTooltipAlignment, true);
}

function handleTooltipEnter(event: Event) {
  const target = tooltipTargetFromEvent(event);
  if (!target) return;

  activeTooltipTarget = target;
  updateTooltipAlignment(target);
}

function handleTooltipLeave(event: Event) {
  const target = tooltipTargetFromEvent(event);
  if (!target || target !== activeTooltipTarget) return;

  const relatedTarget = "relatedTarget" in event ? event.relatedTarget : null;
  if (relatedTarget instanceof Node && target.contains(relatedTarget)) return;

  hideTooltip();
}

function updateActiveTooltipAlignment() {
  if (!activeTooltipTarget || !document.contains(activeTooltipTarget)) {
    hideTooltip();
    return;
  }

  updateTooltipAlignment(activeTooltipTarget);
}

function tooltipTargetFromEvent(event: Event) {
  const target = event.target;
  if (!(target instanceof Element)) return null;
  return target.closest<HTMLElement>(TOOLTIP_SELECTOR);
}

function updateTooltipAlignment(target: HTMLElement) {
  const tooltipText = target.getAttribute("data-tooltip")?.trim() ?? "";
  const viewportWidth = document.documentElement.clientWidth || window.innerWidth;

  if (!tooltipText || viewportWidth <= VIEWPORT_PADDING * 2) {
    hideTooltip();
    return;
  }

  const maxWidth = Math.min(MAX_TOOLTIP_WIDTH, viewportWidth - VIEWPORT_PADDING * 2);
  const { width: tooltipWidth, height: tooltipHeight } = estimateTooltipSize(tooltipText, maxWidth);
  const rect = target.getBoundingClientRect();
  const centerX = rect.left + rect.width / 2;
  const tooltipLeft = clamp(centerX, VIEWPORT_PADDING + tooltipWidth / 2, viewportWidth - VIEWPORT_PADDING - tooltipWidth / 2);
  const belowTop = rect.bottom + TOOLTIP_GAP;
  const hasRoomBelow = belowTop + tooltipHeight + VIEWPORT_PADDING <= window.innerHeight;
  const canFitAbove = rect.top - TOOLTIP_GAP - tooltipHeight >= VIEWPORT_PADDING;
  const showAbove = !hasRoomBelow && canFitAbove;

  const tooltip = ensureTooltipElement();
  const tooltipVisualLeft = tooltipLeft - tooltipWidth / 2;
  const tooltipVisualRight = tooltipVisualLeft + tooltipWidth;
  const arrowLeft = clamp(centerX - tooltipVisualLeft, 12, tooltipVisualRight - tooltipVisualLeft - 12);
  tooltip.element.classList.toggle("global-tooltip-above", showAbove);
  tooltip.element.style.setProperty("--tooltip-left", `${tooltipLeft}px`);
  tooltip.element.style.setProperty("--tooltip-top", `${showAbove ? rect.top - TOOLTIP_GAP - tooltipHeight : belowTop}px`);
  tooltip.element.style.setProperty("--tooltip-max-width", `${maxWidth}px`);
  tooltip.element.style.setProperty("--tooltip-arrow-left", `${arrowLeft}px`);
  tooltip.element.style.setProperty("--tooltip-arrow-top", showAbove ? `calc(100% - 0.25rem)` : "-0.25rem");
  tooltip.element.style.setProperty("--tooltip-arrow-rotation", showAbove ? "225deg" : "45deg");
  tooltip.text.textContent = tooltipText;
  tooltip.element.classList.add("global-tooltip-visible");
}

function hideTooltip() {
  activeTooltipTarget = null;
  tooltipElement?.classList.remove("global-tooltip-visible");
}

function estimateTooltipSize(text: string, maxWidth: number) {
  const rawTextWidth = Array.from(text).reduce((width, character) => {
    if (character === " ") return width + 4;
    return width + (character.charCodeAt(0) > 255 ? 12 : 6.5);
  }, 18);
  const width = Math.min(Math.max(rawTextWidth, 40), maxWidth);
  const lineCount = Math.max(1, Math.ceil(rawTextWidth / Math.max(width, 1)));

  return {
    width,
    height: 14 + lineCount * 15,
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function ensureTooltipElement() {
  if (!tooltipElement || !tooltipTextElement || !tooltipArrowElement) {
    tooltipElement = document.createElement("div");
    tooltipTextElement = document.createElement("div");
    tooltipArrowElement = document.createElement("div");

    tooltipElement.className = "global-tooltip";
    tooltipTextElement.className = "global-tooltip-text";
    tooltipArrowElement.className = "global-tooltip-arrow";
    tooltipElement.append(tooltipArrowElement, tooltipTextElement);
    document.body.append(tooltipElement);
  }

  return {
    element: tooltipElement,
    text: tooltipTextElement,
    arrow: tooltipArrowElement,
  };
}
