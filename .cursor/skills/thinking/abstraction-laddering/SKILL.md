# AI Skill: Abstraction Laddering

## Purpose
Provide an agent the capability to **reframe and explore a problem at multiple levels of abstraction**, enabling both broader context and more concrete solutions.

## When to Use
- Early problem scoping
- Ambiguity resolution
- When the problem statement seems narrow or ill‑defined

## Skill Description
This skill allows the agent to:
- **Move up** (more abstract) by asking *why* questions to uncover broader motivations or system context.
- **Move down** (more concrete) by asking *how* questions to generate actionable forms of the problem.

## Input
- Initial problem description
- Domain context (optional)

## Output
- Multiple reformulations of the problem at varying abstraction levels
- Identification of reframed challenges that surface hidden opportunities

## Technique
Implement an iterative loop:
1. Prompt “Why does this problem matter?” → derive higher‑level problem framing.
2. Prompt “How might this be realized?” → derive operational challenges or solutions.

Store both abstract and concrete problem formulations with metadata for later reasoning reuse.
