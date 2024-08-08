# Validation comparisons

The information contained in these subfolders shows the difference in validation results produced by `gedcomfy` and other validation tools.

The software currently used for comparison are:

**GEDCOM Validator**: Chronoplex’s [GEDCOM Validator](https://chronoplexsoftware.com/gedcomvalidator/) (Version 10.0.4.0 x64, running on .NET 8.0.7)

**GED-inline**: Nigel Parker’s online [GED-inline](https://ged-inline.org/) validator (version 3.1.3). This currently appears to be offline, so I build the command-line application from source. The [source is available on GitHub](https://github.com/nigel-parker/gedinline), but it doesn’t build with current Java tooling; see [my fork for an updated version](https://github.com/Porges/gedinline).

It is important to note that at the moment, `gedcomfy` does not perform any higher-level validation, so the included comparisons are limited to the basic GEDCOM structure. This may make `gedcomfy` look better than it is. Other tooling will be (currently) much better at performing higher-level validation, such as checking for consistency between dates, places, and other data.

## Comparisons

These are the current comparisons:

- [Large Files](comparisons/large_files.md)
