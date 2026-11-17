# Study set item schema (version 1)

Reference format for the items of a study set (GLOSSARY: *Jeu de révision*). It is enforced by the backend (`backend/src/study.rs`: `ItemIn`, `validate_item`) and meant to be produced by humans or by an AI. Unknown fields are rejected.

A set has a `kind`; each kind accepts only some item types:

| set kind | item types |
|---|---|
| `qcm` | `single`, `multi` |
| `questionnaire` | `single`, `multi`, `text` |
| `flashcards` | `flashcard` |
| `summary` | `flashcard` (front = section title, back = its content) |

## Item

```json
{
  "type": "single | multi | text | flashcard",
  "prompt": "string, 1..2000 chars (the question, or the front of a card)",
  "choices": [ { "id": "a", "text": "string, 1..500 chars" } ],
  "answer": { },
  "explanation": "string, optional, up to 4000 chars, shown after correction"
}
```

`id` (uuid) may be added when updating an existing item to keep its history; omit it otherwise.

### `single` : one right choice
2 to 10 `choices`, ids unique, 1..16 chars.
`"answer": { "choice": "b" }`

### `multi` : several right choices (all must be selected, none other)
2 to 10 `choices`. `"answer": { "choices": ["a", "c"] }` (at least one, no duplicate, ids must exist).

### `text` : short free answer
No `choices`. `"answer": { "accepted": ["Paris", "la ville de Paris"] }`. Comparison ignores case, accents, punctuation and extra spaces. With an empty `accepted` list the question is not scored (self-assessed).

### `flashcard`
No `choices`. `"answer": { "back": "string, 1..4000 chars" }`. Not scored; reviewed with spaced repetition (again / hard / good / easy).

## Limits
At most 500 items per set; set title 1..200 chars.

## Example (qcm)

```json
{
  "title": "Réseaux : couche transport",
  "kind": "qcm",
  "items": [
    { "type": "single", "prompt": "Quel protocole est orienté connexion ?",
      "choices": [ { "id": "a", "text": "UDP" }, { "id": "b", "text": "TCP" } ],
      "answer": { "choice": "b" }, "explanation": "TCP établit une connexion (poignée de main en trois temps)." },
    { "type": "multi", "prompt": "Quels ports sont standards ?",
      "choices": [ { "id": "a", "text": "80 (HTTP)" }, { "id": "b", "text": "1234" }, { "id": "c", "text": "443 (HTTPS)" } ],
      "answer": { "choices": ["a", "c"] } }
  ]
}
```

`POST /api/study/sets` accepts this object (plus optional `workspace_id`).

## Submitting an attempt
`POST /api/study/sets/{id}/attempts` with `{ "answers": { "<item uuid>": <value> } }` where value is the choice id (`single`), an array of choice ids (`multi`) or a string (`text`).
