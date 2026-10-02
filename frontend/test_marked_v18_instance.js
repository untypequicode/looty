import { marked, Marked } from 'marked';
console.log(typeof marked.parse);
console.log(typeof Marked);
const instance = new Marked();
console.log(typeof instance.parse);
