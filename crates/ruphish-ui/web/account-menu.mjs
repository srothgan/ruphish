export {bindAnchoredPanel as bindAccountMenu} from './anchored-panel.mjs';

export async function copyAccountText(text) {
  if (!navigator.clipboard?.writeText) throw new Error('Clipboard unavailable');
  await navigator.clipboard.writeText(text);
}
