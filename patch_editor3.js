const fs = require('fs');
let code = fs.readFileSync('frontend/app/components/WysiwygEditor.client.vue', 'utf8');
code = code.replace(
  "provider.connect()",
  "onMounted(() => {\n      provider.connect()\n    })"
);
if (!code.includes("import { useEditor")) {
  console.log("No import found?");
} else if (!code.includes("import { onMounted } from 'vue'")) {
  code = code.replace("import { useEditor", "import { onMounted } from 'vue'\nimport { useEditor");
}
fs.writeFileSync('frontend/app/components/WysiwygEditor.client.vue', code);
