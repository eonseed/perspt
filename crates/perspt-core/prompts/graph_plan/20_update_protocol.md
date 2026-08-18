---
id: graph_plan/update_protocol
version: 1
role: system
required: true
max_bytes: 1024
vars:
  revision_shape: { type: "BoundedText<512>" }
---
Decompose the task into independent work-graph nodes ONLY when parts genuinely touch disjoint files. Call update_graph exactly once; its `revision` argument is JSON: {{revision_shape}}. Declare output_targets precisely; a node without them serializes against everything. Prefer one node when in doubt.
