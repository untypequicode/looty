# The assistant reads files as extracted text, on demand

People want to question the assistant about PDFs, Word documents, Drive files and whole Moodle courses. Providers differ (some accept PDFs natively, Ollama and compatible servers do not), content can be far larger than a prompt, and files are private to people.

Decision: **the server extracts the text of a file and the model reads it through tools, never through the provider's own file input.**

- **Formats**: PDF (with a text layer), `.docx`, `.pptx`, plain text, Markdown, CSV and source code. A file with no text (scanned PDF, image) is refused with an explicit message; there is no OCR for now.
- **Storage**: a file dropped in the chat is saved in the person's personal Drive, folder "Assistant IA", private (ADR 0002), 25 MB at most. It follows the Drive's own expiry and purge; its extracted text is cached with it and removed with it. A conversation that references an expired file shows it as expired, and text already sent in earlier messages stays in the history.
- **Sources**: Drive files (same access check as the Drive, personal items included) and Moodle files and courses (downloaded server-side with the person's own token at send time; never stored beyond the extracted text cache keyed by file).
- **Reading**: an attachment reaches the model as a summary (title, size, pages, outline); small texts (about 12,000 characters) are inlined. Larger ones are read with the tools `read_attachment` (by page or range) and `search_attachment` (keywords), within a per-conversation budget. No embeddings or semantic index.
- **Rights**: everything goes through the person's rights; an attachment gives no write power (ADR 0010).
- **Privacy**: content leaves for the provider the person chose. The first external file attached with a given connection shows a notice once; chips mention the provider.

Why: works with every provider and local models, one code path, no duplicated copy of private documents, and a predictable cost (the model only reads what it asks for).

Cost: scanned documents and images are not supported yet; reading by keyword is less clever than a semantic index.
