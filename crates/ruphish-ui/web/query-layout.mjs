// Keep the last rendered height only while a different selection is loading.
// Query state remains owned by Rust; this observer stores layout measurements only.
export function retainQueryHeight(region, content) {
  const observer = new ResizeObserver(() => {
    if (region.dataset.ready !== 'true' || content.hidden) {
      return;
    }
    const height = content.getBoundingClientRect().height;
    if (height > 0) {
      region.style.setProperty('--query-content-height', `${height}px`);
    }
  });
  observer.observe(content);
  return () => observer.disconnect();
}
