# rimelight_content

Content model for Rimelight pages: the block registry, rich text, and the Loro document mapping. Used by the native
CMS editor (Space) and mirrored by the content service in the website repo.

- `MAPPING.md` is the contract. Both sides implement it and are tested on the same `fixtures/`.
- `registry/blocks.json` is the single source of block definitions. The TypeScript side copies it; do not edit a copy.
- `fixtures/` holds the shared test pages in normal form.

This crate has no UI dependency.
