
Main architectural weaknesses
1. Tool sprawl will hurt agent reliability

You currently expose many capabilities and then use semantic routing to select top-K tools. That is reasonable, but it is also fragile. Reddit/AI-builder discussions increasingly warn that too many independent tools make the model spend cognition on tool choice instead of task completion. One recent LocalLLaMA discussion makes the exact point: more tools increases selection difficulty and drops accuracy.

Reddit

•

r/LocalLLaMA

›

Before each call, the LLM must make a tool selection — which one? What parameters? The more tools you add, the harder the selection, and accuracy drops.

Give feedback

Fix: keep semantic routing, but add a second layer: task-scoped tool profiles.

Instead of:

user request → semantic router → top-K capabilities → agent loop

Use:

user request
→ intent class
→ task profile
→ allowed capability set
→ semantic router within that set
→ execution

Example profiles:

Task profile	Allowed tools
document extraction	OCR, invoice, CV, medical claim, classifier
workspace editing	file read/write, versions, search
email composition	compose-email, report-md
admin/debug	runtime-echo, capability test, reload
upload pipeline	sense-mime, convert-pdf, classifier, plan-on-upload

Semantic routing across everything is elegant. Elegance is how bugs dress for court.