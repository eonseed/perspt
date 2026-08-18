---
id: branch_correct/correction
version: 1
role: user
required: true
max_bytes: 16384
vars:
  cluster_summary: { type: "BoundedText<1024>" }
  diagnostics: { type: "BoundedList<32,512>", style: bullet_list }
  paths: { type: "BoundedList<32,256>", style: comma_list }
  symbols: { type: "BoundedList<32,128>", style: comma_list }
  operators: { type: "BoundedList<8,1024>", style: bullet_list }
---
Dominant cluster: {{cluster_summary}}
Diagnostics:
{{diagnostics}}
Affected paths:
{{paths}}
Affected symbols:
{{symbols}}
Correction operators:
{{operators}}
