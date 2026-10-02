import { Marked } from 'marked';
const marked = new Marked();
marked.use({
  async walkTokens(token) {
    if (token.type === 'image') {
      token.href = await Promise.resolve('uploaded.jpg');
    }
  }
});
const html = await marked.parse('![](attachments/img.jpg " =100x100")');
console.log(html);
