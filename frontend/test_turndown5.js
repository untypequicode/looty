import TurndownService from 'turndown';
import { JSDOM } from 'jsdom';

const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>');
global.document = dom.window.document;
global.window = dom.window;
global.HTMLElement = dom.window.HTMLElement;

const turndownService = new TurndownService({
  blankReplacement: function (content, node) {
    if (node.nodeName === 'SPAN' && node.getAttribute('data-type') === 'math-inline') {
      return '$' + (node.getAttribute('latex') || '') + '$'
    }
    if (node.nodeName === 'DIV' && node.getAttribute('data-type') === 'math-block') {
      return '\n$$\n' + (node.getAttribute('latex') || '') + '\n$$\n'
    }
    return node.isBlock ? '\n\n' : ''
  }
});

console.log('OUTPUT:', turndownService.turndown('<p><span latex="x_i" data-type="math-inline"></span> hello <div latex="y" data-type="math-block"></div></p>'));
