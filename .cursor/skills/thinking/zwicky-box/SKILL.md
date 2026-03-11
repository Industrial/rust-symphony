# AI Skill: Zwicky Box Combinatorial Generation

## Purpose
Generate **combinatorially broad solution sets** for complex, multidimensional problems by exploring configurations of attributes.

## When to Use
- Creative ideation
- Large configuration spaces
- Innovation challenges where typical brainstorming fails

## Skill Description
This skill allows an agent to:
- **Identify independent attributes** of a problem.
- Populate each attribute with possible values.
- **Systematically enumerate combinations** to discover novel solution hypotheses.

## Input
- Problem definition with candidate attributes
- Possible values or variable domains for each attribute

## Output
- Set of candidate solution combinations
- Ranked or filtered list of viable configurations

## Technique
1. Decompose the problem into orthogonal dimensions.
2. Populate a matrix (“box”) where each column = attribute, each row = value.
3. Generate combinations via Cartesian product; apply heuristics or constraints to prune irrelevant combinations.

Use combinatorial generation and constraint filters to maintain a manageable search space.
