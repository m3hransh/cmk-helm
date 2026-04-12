# Hackerney Docs - Obsidian Vault

This is the project knowledge base for the cmk-cockpit, built as an Obsidian vault following the Zettelkasten method.

## Vault Structure

```
notes/           # All notes — flat, no subdirectories
Templates/       # Obsidian note templates (Templater)
Archive/         # Obsolete documentation
```

All notes live in a single flat `notes/` directory. Organisation comes from **tags and links**, not folder hierarchy. This is core Zettelkasten: atomic notes connected by wikilinks, categorised by frontmatter tags.
Try to use as many links as possible to connect notes together. This creates a rich web of knowledge that can be easily navigated and queried with Dataview.

## Tagging System

### Tag categories (frontmatter `tags` field):
- **`concept`** — Technical concepts and knowledge
- **`tool`** — Tool and library documentation
- **`architecture`** — System design and architecture decisions (use mermaid for diagrams)
- **`app`** — App overview notes
- **`guide`** — How-to guides and tutorials
- **`phase`** — Project development phases
- **`feature`** — Feature documentation
- **`workflow`** — Development and testing workflows
- **`deployment`** — Deployment and operations docs

### Status values (frontmatter):
- `backlog` — not started
- `inprogress` — actively being worked on
- `completed` — finished
- `archived` — no longer relevant

### Priority values (frontmatter):
- `3` — high
- `2` — medium (default)
- `1` — low

## How to Write Notes

### When creating a new note:
1. Pick the matching template from `Templates/`
2. Place file in `notes/` (always flat — never create subdirectories)
3. Use the correct `tags` value in frontmatter
4. Default: `status: backlog`, `priority: 2`, `publish: true`
5. Use `[[wikilinks]]` for all internal references
6. Include a `## Metadata` section at the bottom with Tags, Reference, Related fields

### Formatting rules:
- Always use YAML frontmatter with tags, status, priority, publish, aliases
- Use `[[wikilinks]]` for internal links — no folder paths needed since everything is in `notes/`
- File names should be descriptive and title-cased
- Use Templater syntax (`<%tp.*%>`) only in Templates, not in actual notes

## Dataview Queries
Create directory Query and try query based on different things for instance notes related to OTel or
based on Types of notes like concept, feature, phase etc.

```dataview
TABLE status, priority FROM #concept SORT priority DESC
```

```dataview
TABLE status FROM #feature WHERE status != "archived"
```

```dataview
LIST FROM #phase SORT file.name ASC
```

## Plugins in Use
- **Templater** — template engine for note creation
- **Dataview** — SQL-like queries over notes

## Key Entry Points
