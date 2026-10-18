// Protect Go actions before any HTML parser can reinterpret quotes or operators.
export class TemplateVault {
  constructor(source) {
    this.prefix = 'RUPHISH_TEMPLATE_';
    while (source.includes(this.prefix)) this.prefix += 'X';
    this.actions = [];
  }

  mask(source) {
    let output = '', cursor = 0;
    while (cursor < source.length) {
      const start = source.indexOf('{{', cursor);
      if (start < 0) return output + source.slice(cursor);
      output += source.slice(cursor, start);
      let quote = null, comment = false, end = -1;
      for (let index = start + 2; index < source.length; index++) {
        const pair = source.slice(index, index + 2);
        const character = source[index];
        if (comment) { if (pair === '*/') { comment = false; index++; } continue; }
        if (quote) {
          if (character === '\\' && quote !== '`') { index++; continue; }
          if (character === quote) quote = null;
          continue;
        }
        if (pair === '/*') { comment = true; index++; }
        else if (['"', "'", '`'].includes(character)) quote = character;
        else if (pair === '}}') { end = index + 2; break; }
      }
      if (end < 0) throw new Error('Unclosed template expression. Keep editing this document in source mode.');
      const marker = `${this.prefix}${this.actions.length}_END`;
      this.actions.push({marker, source: source.slice(start, end)});
      output += marker;
      cursor = end;
    }
    return output;
  }

  restore(source) {
    // One pass prevents a restored expression from being treated as another marker.
    const known = new Map(this.actions.map(action => [action.marker, action.source]));
    const output = source.replace(new RegExp(`${this.prefix}\\d+_END`, 'g'), marker => {
      if (!known.has(marker)) throw new Error('An unknown template marker was introduced. The source was kept.');
      return known.get(marker);
    });
    // The prefix does not occur in original content; an incomplete marker is an edit error.
    const withoutKnown = source.replace(new RegExp(`${this.prefix}\\d+_END`, 'g'), '');
    if (withoutKnown.includes(this.prefix)) throw new Error('A template field was partially edited. Undo that edit or use source mode.');
    return output;
  }

  decorate(document) {
    const walker = document.createTreeWalker(document.body, 4);
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const node of nodes) {
      if (node.parentElement?.closest('script,style,textarea,[data-ruphish-field]')) continue;
      const matches = [...node.data.matchAll(new RegExp(`${this.prefix}\\d+_END`, 'g'))];
      if (!matches.length) continue;
      const fragment = document.createDocumentFragment();
      let start = 0;
      for (const match of matches) {
        const action = this.actions.find(action => action.marker === match[0]);
        if (!action) throw new Error('Unknown template field');
        fragment.append(document.createTextNode(node.data.slice(start, match.index)));
        const chip = document.createElement('span');
        chip.setAttribute('data-ruphish-field', action.marker);
        chip.setAttribute('contenteditable', 'false');
        chip.textContent = action.source;
        fragment.append(chip);
        start = match.index + match[0].length;
      }
      fragment.append(document.createTextNode(node.data.slice(start)));
      node.replaceWith(fragment);
    }
  }

  export(document) {
    const copy = document.cloneNode(true);
    for (const chip of copy.querySelectorAll('[data-ruphish-field]')) {
      const marker = chip.getAttribute('data-ruphish-field');
      if (!marker.startsWith(this.prefix)) continue;
      if (!this.actions.some(action => action.marker === marker)) throw new Error('Unknown template field');
      chip.replaceWith(copy.createTextNode(marker));
    }
    return this.restore(copy.documentElement.outerHTML);
  }
}
