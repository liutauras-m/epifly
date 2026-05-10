/**
 * Lightweight markdown → safe HTML converter.
 * Handles the common subset Claude produces: headings, bold, italic,
 * inline-code, fenced code blocks, ordered/unordered lists, paragraphs.
 * All raw HTML in the source text is escaped before processing.
 */

function esc(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function inlineStyles(s: string): string {
  return s
    // bold+italic
    .replace(/\*\*\*([^*\n]+)\*\*\*/g, '<strong><em>$1</em></strong>')
    // bold
    .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>')
    // italic (single * not touching whitespace boundary)
    .replace(/(?<![*\w])\*([^*\n]+)\*(?![*\w])/g, '<em>$1</em>')
    // inline code
    .replace(/`([^`\n]+)`/g, '<code>$1</code>');
}

export function renderMarkdown(raw: string): string {
  // 1. Extract fenced code blocks before any escaping so backticks survive.
  const blocks: string[] = [];
  const withPlaceholders = raw.replace(/```(\w*)\n?([\s\S]*?)```/g, (_, lang, code) => {
    const langAttr = lang ? ` class="language-${esc(lang)}"` : '';
    blocks.push(`<pre class="md-pre"><code${langAttr}>${esc(code.replace(/\n$/, ''))}</code></pre>`);
    return `\x00BLOCK${blocks.length - 1}\x00`;
  });

  // 2. Escape remaining HTML.
  const escaped = esc(withPlaceholders);

  // 3. Split into lines and process block-level constructs.
  const lines = escaped.split('\n');
  const out: string[] = [];
  let i = 0;
  let listStack: Array<{ type: 'ul' | 'ol'; indent: number }> = [];

  function closeLists() {
    while (listStack.length) {
      out.push(`</${listStack.pop()!.type}>`);
    }
  }

  while (i < lines.length) {
    const line = lines[i];

    // Headings
    const h = line.match(/^(#{1,3})\s+(.+)$/);
    if (h) {
      closeLists();
      const level = h[1].length;
      out.push(`<h${level} class="md-h${level}">${inlineStyles(h[2])}</h${level}>`);
      i++;
      continue;
    }

    // Horizontal rule
    if (/^---+$/.test(line.trim())) {
      closeLists();
      out.push('<hr class="md-hr">');
      i++;
      continue;
    }

    // Unordered list item
    const ul = line.match(/^(\s*)[-*]\s+(.+)$/);
    if (ul) {
      const indent = ul[1].length;
      if (!listStack.length || listStack[listStack.length - 1].type !== 'ul') {
        closeLists();
        out.push('<ul class="md-ul">');
        listStack.push({ type: 'ul', indent });
      }
      out.push(`<li>${inlineStyles(ul[2])}</li>`);
      i++;
      continue;
    }

    // Ordered list item
    const ol = line.match(/^(\s*)\d+\.\s+(.+)$/);
    if (ol) {
      const indent = ol[1].length;
      if (!listStack.length || listStack[listStack.length - 1].type !== 'ol') {
        closeLists();
        out.push('<ol class="md-ol">');
        listStack.push({ type: 'ol', indent });
      }
      out.push(`<li>${inlineStyles(ol[2])}</li>`);
      i++;
      continue;
    }

    // Blank line — close open lists, emit paragraph break sentinel
    if (line.trim() === '') {
      closeLists();
      out.push('\x01');
      i++;
      continue;
    }

    // Regular text line
    closeLists();
    out.push(inlineStyles(line));
    i++;
  }

  closeLists();

  // 4. Join lines, splitting on \x01 (blank-line) into <p> paragraphs.
  const joined = out.join('\n');
  const paragraphs = joined
    .split(/\n?\x01\n?/)
    .map(s => s.trim())
    .filter(Boolean);

  const html = paragraphs
    .map(p => {
      // Block-level elements don't need <p> wrapping.
      if (/^<(h[1-6]|ul|ol|pre|hr)/.test(p)) return p;
      return `<p class="md-p">${p.replace(/\n/g, '<br>')}</p>`;
    })
    .join('\n');

  // 5. Restore code block placeholders.
  return html.replace(/\x00BLOCK(\d+)\x00/g, (_, idx) => blocks[Number(idx)]);
}
