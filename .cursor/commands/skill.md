# Use the right skill for this task

You are using the **/skill** command. The user has provided a task or topic (in this message or the text that follows). Your job is to pick the best-matching skill and apply it.

## What to do

1. **Identify the task**  
   Treat the user's full message as the task (e.g. "write a web app", "debug this test", "add Docker", "review this PR").

2. **Choose the right skill(s)**  
   From the **available skills** listed in your context (the `agent_skills` section), pick the skill(s) whose description and "Use when" best match the task. Prefer one primary skill; add others only if the task clearly needs them.

3. **Read the skill**  
   Use the **Read** tool with the skill's `fullPath` to load the full SKILL.md. Do this before answering or writing code.

4. **Follow the skill**  
   Do the task by following that skill's instructions. If the skill says "use when…", you are in that situation now—apply it.

5. **Respond**  
   Reply in terms of the task (e.g. "Here's the plan…" or "I'll use the X skill and…"), then do the work as the skill specifies.

## Rules

- **Always** read the chosen skill file before acting; do not rely only on the short description.
- If no skill fits well, say so and either do the task with your best judgment or suggest creating a new skill.
- If several skills fit, pick the most specific one first, or the one that best matches the main goal of the request.
