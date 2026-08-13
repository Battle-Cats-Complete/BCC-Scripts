# Neko-Arc
Switch assets are stored under a 64-bit hash of their full asset path (`Data/Image/{name}.png.bntx` for art, `Data/CsvFiles/{name}.csb` for `.csv` and `.tsv` tables), so an unpacked archive is a folder of hex names until it gets a name source to match against.

### DECRYPT
`neko-arc decrypt <FILE | DIR>...`

Extracts every member of an `.arc` into a folder named after the archive in the working directory. Members are written as `{hash}.bntx`, `{hash}.csb`, or extension-less when the payload carries no recognizable magic.

Takes as many archives as you care to name, and accepts `--file` (`-f`), repeatable, and `--dir` (`-d`) `<DIR> <LEVEL>` the same way the other commands do. Walking a directory picks up `.arc` files only; a file you name yourself is always tried, whatever it is called. One bad archive is reported and skipped rather than ending the run.

`--output` (`-o`) is the destination folder for a single archive. Given several, it becomes the parent they each get a folder inside, so nothing overwrites anything else.

### DECODE
`neko-arc decode <FILE | DIR>`

Turns original names into hashes and, optionally, renames the unpacked files to match. A bare input with no other flags behaves as `--file` plus `--print`.

- `--file` (`-f`) `<FILE>` works with one name.
- `--dir` (`-d`) `<DIR> <LEVEL>` hashes every file name inside a directory, walking `LEVEL` deep (default `10`).
- `--dictionary <FILE>` reads a prebuilt `{hash},{name}` list and skips hashing entirely, only touching files whose name is one of those hashes.
- `--print` (`-p`) prints every name next to its hash.
- `--rename` (`-r`) `<LEVEL>` walks the working directory `LEVEL` deep (default `10`) and renames matching hashed files, keeping the container extension (`{hash}.bntx` becomes `{name}.png.bntx`).

### DICTIONARY
`neko-arc dictionary <DIR> <LEVEL>`

Writes a `{hash},{name}` pair per line for every file found under `DIR`, walking `LEVEL` deep (default `10`). Lands in `dictionary.csv` in the working directory unless `--output` (`-o`) points elsewhere.

### CONVERT
`neko-arc convert <FILE | DIR>`

Converts `BNTX` to `PNG` and `CSB` to `CSV` / `TSV`, replacing the originals in place. Accepts `--file` (`-f`) and `--dir` (`-d`) `<DIR> <LEVEL>` the same way `decode` does. Pass `--output` (`-o`) to dump the results into a directory instead and leave the originals alone.

Reads block-linear and linear Tegra tiling, every `BCn` and `ASTC` block format, and the packed colour formats, then applies the texture's own channel selectors.

Tables keep whatever name `decode` resolved for them, so `uni.imgcut.csb` becomes `uni.imgcut` and `stage.tsv.csb` becomes a tab-separated `stage.tsv`. Only `.tsv` uses tabs; every other table is comma-separated, which covers `.csv` alongside the `.imgcut`, `.mamodel`, and `.maanim` rigging formats. A file still under its hash has no name to go by, so the delimiter is picked from its contents and the matching extension is appended.

### FIX
`neko-arc fix <FILE | DIR>`

The lossy counterpart to `convert`. Everything here trades exact fidelity for data you can actually load, so reach for it only once you have decided you want that. Takes `--file` (`-f`) or `--dir` (`-d`) `<DIR> <LEVEL>` and repairs in place.

Each kind is opted into separately, because two of them consume their input:

- `--png` un-premultiplies alpha on `PNG` files, clearing the gray fringing that `BNTX` conversion leaves behind. Every game texture is premultiplied, so this applies to every semi-transparent pixel it finds; images made up entirely of fully opaque or fully transparent pixels come out untouched. This is what runs when no kind is chosen.
- `--bntx` decodes a container straight to a corrected image in one step, replacing the `.bntx`. Same result as `convert` followed by `--png`, without ever writing the premultiplied version to disk.
- `--csb` writes tables out the way `convert` does, except any field holding a line break or the table's own separator has those characters flattened to spaces, replacing the `.csb`. Without it a single field can silently split one row into several, or invent columns that were never there. Only the internal design spreadsheets that shipped with multi-line header cells need this.
- `--all` does every kind in one pass.

Un-premultiplying is not reversible, and an already-corrected image is indistinguishable from one that still needs it, so `--png` is a one-shot pass: run it over the same images twice and it will brighten them twice. Use `--bntx` on a freshly extracted tree when you want that decision taken out of your hands, since decoding and correcting in one step cannot double-apply.
