const regex = /^#\s+(?:([\p{Emoji_Presentation}\p{Extended_Pictographic}])\s+)?(.*)/u;
const tests = [
  "# 🥰 Introduction à la",
  "# Introduction à la",
  "# 🚀 Test",
  "# Test"
];
for (const t of tests) {
  const match = t.match(regex);
  console.log(t, "->", match ? `Emoji: ${match[1]}, Title: ${match[2]}` : "No match");
}
