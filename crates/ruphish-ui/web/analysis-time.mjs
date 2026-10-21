export function observeAnalysisWidth(element, callback) {
    const observer = new ResizeObserver(entries => callback(entries[0].contentRect.width));
    observer.observe(element);
    return observer;
}
export function unobserveAnalysisWidth(observer) { observer.disconnect(); }
