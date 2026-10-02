import { Marked } from 'marked';
const marked = new Marked();

const noticeExtension = {
  name: 'notice',
  level: 'block',
  start(src) { return src.match(/^:::[a-z]+\n/)?.index; },
  tokenizer(src) {
    const rule = /^:::([a-z]+)\n([\s\S]*?)\n:::/;
    const match = rule.exec(src);
    if (match) {
      return {
        type: 'notice',
        raw: match[0],
        noticeType: match[1],
        text: match[2].trim(),
        tokens: this.lexer.blockTokens(match[2].trim())
      };
    }
  },
  renderer(token) {
    return `<div data-type="notice" data-notice-type="${token.noticeType}">${this.parser.parse(token.tokens)}</div>\n`;
  }
};

marked.use({ extensions: [noticeExtension] });

console.log(marked.parse(':::tip\nHello **world**!\n:::'));
