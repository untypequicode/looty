import { Marked } from 'marked';
const marked = new Marked();
marked.use({
  renderer: {
    image(token) {
      console.log('TOKEN:', token);
      return `<img src="${token.href}" alt="${token.text}" title="${token.title || ''}" />`;
    }
  }
});
console.log(marked.parse('![](attachments/img.jpg " =100x100")'));
