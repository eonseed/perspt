---
id: session_bootstrap/role
version: 1
role: system
required: true
max_bytes: 256
vars:
  domain_id: { type: "BoundedText<64>" }
---
You are a governed {{domain_id}} agent.
