# R runs on the project's Jupyter server only; R Markdown converts to notebooks

Issue #237 asks for R cells next to Python. ADR 0003 and 0008 already decided that Looty executes no code on its own server and that the browser engine is Pyodide (Python).

Decision:
- **R runs only through the project's Jupyter server** (IRkernel, chosen as the notebook's kernel). The language of a notebook is read from its `kernelspec` / `language_info`; Looty adds nothing new on the backend. Running an R notebook with Pyodide is refused with an explicit message.
- **WebR is not shipped for now**: it would add a large WebAssembly payload (R runtime and packages) for a browser engine, and the Jupyter relay covers the need. It stays an open option, tracked in the issue.
- **R Markdown / Quarto documents are converted, not run**: `.Rmd` and `.qmd` convert to and from `.ipynb` in the browser (`editor/rmarkdown.ts`), jupytext-style. The exact chunk header, fence length, blank lines, line endings and YAML delimiters are kept in `metadata.looty.rmd`, so a round trip gives back the same text. Outputs and Markdown attachments are not written to `.Rmd` / `.qmd`.
- Chunk options are interpreted only for rendering exports (`echo`, `include`, `results`, Quarto `#|` lines); execution ignores them.

Consequences: no new server endpoint, no stored secret. The RStudio-like panels (environment, plots, help, console) are not built yet; they need silent kernel requests outside cells.

Update (issue #239): the RStudio-like panels now exist (`components/notebook/RPanels.vue`, `editor/rPanels.ts`). They use **kernel queries outside cells**: an `execute_request` with `store_history: false` (no execution number, no cell), whose output goes back to the panel by its `parent_header` and is never written to the notebook. Environment and help queries are silent: they do not start a kernel, and are skipped if it is not connected. The console runs with history on the same kernel. Plot history is read from the cells' outputs and the console's, not from the kernel. The R code sent is fixed text; names typed by people are always quoted as R strings. Not covered: the panels need the Jupyter engine (not Pyodide), variables are listed from the global environment only, and a long calculation delays the answer (requests queue behind it, 15 to 20 s then an explicit timeout).
