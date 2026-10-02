import TurndownService from 'turndown';
import { JSDOM } from 'jsdom';

const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>');
global.document = dom.window.document;
global.window = dom.window;
global.HTMLElement = dom.window.HTMLElement;

const turndownService = new TurndownService();
turndownService.addRule('math-inline', {
    filter: function (node) {
      console.log('NODE:', node.nodeName, node.getAttribute('data-type'), node.getAttribute('latex'));
      return node.nodeName === 'SPAN' && node.getAttribute('data-type') === 'math-inline'
    },
    replacement: function (content, node) {
      const latex = node.getAttribute('latex') || ''
      console.log('REPLACEMENT LATEX:', latex);
      return '$' + latex + '$'
    }
  });

console.log('OUTPUT:', turndownService.turndown('<p><span latex="x_i" data-type="math-inline"></span></p>'));
