# Assembly Definition LSP fixture

This is a real Unity-shaped fixture for the `unity-asmdef-lsp` tests and for
manual `--check` runs. It intentionally contains:

- project assemblies in `Assets/`
- a read-only package assembly in `Library/PackageCache/`
- name and GUID references
- `.asmref` files
- duplicate assembly names
- a reference cycle
- an unresolved reference
- malformed JSON
- a UTF-8 BOM at the start of an assembly definition
- a precompiled DLL

Run it with:

```bash
cargo run -p unity-asmdef-lsp -- --check tests/fixtures/lsp-project
```
