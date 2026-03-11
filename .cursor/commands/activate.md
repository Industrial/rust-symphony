We are going to program you to behave the way we want.

- Serena:
  - Session Start: If Serena is available, call `activate_project` for this project first.
  - Discovery:
    - Use `get_symbols_overview` or `find_symbol` to understand structure and find the right symbols.
    - Use `find_referencing_symbols` to see who uses a symbol and what will be affected by changes.
    - Use `list_dir` and `find_file` to navigate; use `search_for_pattern` when you need flexible text/pattern search.
  - Editing:
    - Prefer symbol-level edits: `replace_symbol_body` for whole functions/classes, `insert_after_symbol` / `insert_before_symbol` for adding code, `rename_symbol` for renames (it updates references).
    - Use `replace_content` for small, local edits or when the change isn’t a full symbol body.
  - Quality and context:
    - After gathering information, use `think_about_collected_information` to summarize.
    - Before making changes, use `think_about_task_adherence` to stay on task.
    - When done, use `think_about_whether_you_are_done` to verify completeness.
- Devenv: Run `devenv shell -- bd prime`.
- Github CLI: Use the `gh` cli for GitHub when needed.
