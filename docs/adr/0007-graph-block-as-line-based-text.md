# The graph block is Cytoscape over a line-based text source

Users draw a lot of graphs (flow networks, shortest paths, trees, automata) and wanted to edit them with the mouse, which Mermaid cannot do: it always lays nodes out itself, and its graph syntax is built for flowcharts, not weighted or undirected graphs. The alternatives were draw.io embedded in an iframe (heavy, outside the collaborative document, no notion of weights or flows), Vue Flow (made for node-and-port pipeline editors), Graphviz (text only) and Cytoscape.js (a graph-theory library: draggable nodes, directed and undirected edges, labels, layouts, algorithms; MIT; already in the bundle as a Mermaid dependency).

Decision: **a separate graph block, drawn with Cytoscape.js, whose source is the node's own text, one line per node or edge** (`s : Source @ 0 100`, `s -> t : 3/5`, `a -- b`, a trailing `*` to highlight), like Mermaid and LaTeX sources (see `frontend/app/editor/source.ts`). Mouse edits only rewrite the lines they touch: moving a node changes its line, connecting two nodes appends one. The style and the block height are node attributes. Mermaid stays for flowcharts, sequences and Gantt charts.

Why:
- Yjs merges the text character by character: two people moving different nodes, or one dragging while another types an edge in the code view, do not overwrite each other. A JSON attribute would keep only the last write.
- The text is readable and writable by hand, survives the Markdown export as a `looty-graph` code block and the Outline import/export, and needs no migration when the visual editor changes.
- Nodes without a position are placed automatically and their position is only written when someone drags them or runs **Arrange**: a hand-written graph does not fill up with coordinates on its own.

Consequences: adding the `graph` node raised the document format to `DOC_SCHEMA = 3`, so tabs opened before must reload to edit. The format is now part of every document that uses the block: renaming its syntax later means converting stored pages. Two people dragging the same node still end with the last position. Cytoscape draws on a canvas, so the stored HTML only holds the text source: the Markdown export and outside tools get the text, not a picture (pages in Looty, public links included, draw it).
