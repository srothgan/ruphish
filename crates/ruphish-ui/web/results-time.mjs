export function observeResultsWidth(element, callback) {
    const observer = new ResizeObserver(entries => callback(entries[0].contentRect.width));
    observer.observe(element);
    return observer;
}
export function unobserveResultsWidth(observer) { observer.disconnect(); }
