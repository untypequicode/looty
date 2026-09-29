const fs = require('fs');
let code = fs.readFileSync('frontend/app/components/WysiwygEditor.client.vue', 'utf8');
code = code.replace(
  "editorOptions.content = '<p></p>'",
  "// editorOptions.content = '<p></p>'"
);
fs.writeFileSync('frontend/app/components/WysiwygEditor.client.vue', code);
