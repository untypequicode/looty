const fs = require('fs');
let code = fs.readFileSync('frontend/app/components/WysiwygEditor.client.vue', 'utf8');
code = code.replace(
  "const editor = useEditor(editorOptions)",
  "const editor = ref(null)\nconst initEditor = () => { editor.value = useEditor(editorOptions) }"
);
code = code.replace(
  "provider.on('synced', () => {",
  "provider.on('synced', () => {\n    initEditor()"
);
code = code.replace(
  "if (!props.documentId) {\n  editorOptions.content = props.modelValue\n} else {\n  // editorOptions.content = '<p></p>'\n}",
  "if (!props.documentId) {\n  editorOptions.content = props.modelValue\n  initEditor()\n}"
);
fs.writeFileSync('frontend/app/components/WysiwygEditor.client.vue', code);
