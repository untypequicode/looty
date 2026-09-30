fetch("http://localhost:3001/api/boards/tasks/38321285-2efc-4606-a8e9-0b1a0030d34d", {
  method: "PUT",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ title: "tache 5", is_per_person: true })
}).then(res => res.text()).then(console.log).catch(console.error);
