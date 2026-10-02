import { marked } from 'marked';
import fs from 'fs';

const mathBlockExtension = {
  name: 'math-block',
  level: 'block',
  start(src) { return src.match(/\$\$/)?.index; },
  tokenizer(src, tokens) {
    const rule = /^\$\$([^$]+)\$\$/;
    const match = rule.exec(src);
    if (match) {
      return {
        type: 'math-block',
        raw: match[0],
        text: match[1].trim()
      };
    }
  },
  renderer(token) {
    const escapedLatex = token.text.replace(/"/g, '&quot;');
    return `<div data-type="math-block" latex="${escapedLatex}"></div>`;
  }
};

const mathInlineExtension = {
  name: 'math-inline',
  level: 'inline',
  start(src) { return src.match(/\$/)?.index; },
  tokenizer(src, tokens) {
    const rule = /^\$([^$\n]+)\$/;
    const match = rule.exec(src);
    if (match) {
      return {
        type: 'math-inline',
        raw: match[0],
        text: match[1].trim()
      };
    }
  },
  renderer(token) {
    const escapedLatex = token.text.replace(/"/g, '&quot;');
    return `<span data-type="math-inline" latex="${escapedLatex}"></span>`;
  }
};

marked.use({ extensions: [mathBlockExtension, mathInlineExtension] });
const text = fs.readFileSync('test.md', 'utf8');
const html = marked.parse(text);
console.log(html);
