This is a GEDCOM parser that got out of hand.

The various crates are:

- `mdf`: the top-level tool for dealing with GEDCOM files
- `gedcomfy`: the GEDCOM parser & schemas itself
- `gedcomesque`: SQL types for GEDCOM
- [`errful`](./errful/README.md): supplementary information for errors (like `miette`) and rendering
- `errful-derive`: derive proc-macro for `errful`	
- `snippets`: rendering source code with labels attached
- `complex-indifference`: typed numeric types

### Dependency Graph

```mermaid
graph TD;
    mdf --> gedcomfy;
    mdf --> complex-indifference;
    gedcomfy --> errful;
    gedcomfy --> complex-indifference;
    errful --> errful-derive;
    errful --> snippets;
    errful --> complex-indifference;
    snippets --> complex-indifference;
    gedcomesque --> gedcomfy;
```
