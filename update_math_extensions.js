import fs from 'fs';

const inlineExtFile = '/home/untype/Development/looty-2/frontend/app/components/CustomMathInlineExtension.js';
let inlineCode = fs.readFileSync(inlineExtFile, 'utf8');

if (!inlineCode.includes('nodePasteRule')) {
  inlineCode = inlineCode.replace(
    "import { InputRule } from '@tiptap/core'",
    "import { InputRule, pasteRule } from '@tiptap/core'"
  );
  
  const pasteRuleCode = `
  addPasteRules() {
    return [
      pasteRule({
        find: /(?:^|\\s)\\$([^$\\n]+)\\$(?!\\d)/g,
        handler: ({ state, range, match }) => {
          const { tr } = state;
          const start = range.from;
          const end = range.to;
          const latex = match[1].trim();
          tr.replaceWith(start, end, this.type.create({ latex }));
        },
      }),
    ]
  },
`;
  inlineCode = inlineCode.replace("addCommands() {", pasteRuleCode + "\n  addCommands() {");
  fs.writeFileSync(inlineExtFile, inlineCode);
}

const blockExtFile = '/home/untype/Development/looty-2/frontend/app/components/CustomMathExtension.js';
let blockCode = fs.readFileSync(blockExtFile, 'utf8');

if (!blockCode.includes('pasteRule')) {
  blockCode = blockCode.replace(
    "import { VueNodeViewRenderer } from '@tiptap/vue-3'",
    "import { VueNodeViewRenderer } from '@tiptap/vue-3'\nimport { pasteRule } from '@tiptap/core'"
  );
  
  const pasteRuleCode = `
  addPasteRules() {
    return [
      pasteRule({
        find: /\\$\\$([^$]+)\\$\\$/g,
        handler: ({ state, range, match }) => {
          const { tr } = state;
          const start = range.from;
          const end = range.to;
          const latex = match[1].trim();
          tr.replaceWith(start, end, this.type.create({ latex }));
        },
      }),
    ]
  },
`;
  blockCode = blockCode.replace("addNodeView() {", pasteRuleCode + "\n  addNodeView() {");
  fs.writeFileSync(blockExtFile, blockCode);
}
