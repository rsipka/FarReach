#!/usr/bin/env python3
"""Build src/lib/data/terminal-schemes.json from iTerm2-Color-Schemes.

    git clone https://github.com/mbadolato/iTerm2-Color-Schemes /tmp/its
    python3 scripts/gen-terminal-schemes.py /tmp/its

A scheme comes from its .itermcolors file, or from its YAML source when the
collection has only that. Only schemes whose entry in the collection's licenses.yml names a permissive
licence are taken: unknown, non-free and copyleft ones are left out, and can
still be imported by hand from their .itermcolors file, or downloaded from
the collection by name in Settings (the names are in the file's "more" list). Each scheme is stored
as its name and its 22 colours in TERMINAL_KEYS order (see
src/lib/state/terminal-themes.svelte.ts), to keep the file small. The
licence and source of every scheme go to src/lib/data/terminal-schemes-licenses.md.
"""
import json
import plistlib
import subprocess
import sys
from pathlib import Path

import yaml

PERMISSIVE = {'MIT', 'Apache-2.0', 'ISC', 'CC0-1.0', 'WTFPL', 'Zlib', 'Unlicense', '0BSD',
              'BSD-2-Clause', 'BSD-3-Clause', 'MIT-0'}

# The order of TerminalPalette's keys in TERMINAL_KEYS.
KEYS = ['Background Color', 'Foreground Color', 'Cursor Color', 'Cursor Text Color',
        'Selection Color', 'Selected Text Color'] + [f'Ansi {i} Color' for i in range(16)]


def permissive(licence: str) -> bool:
    parts = [p.strip() for p in licence.replace(' AND ', ' OR ').split(' OR ')]
    return all(p in PERMISSIVE for p in parts)


def hex_of(c: dict) -> str:
    rgb = [round(max(0.0, min(1.0, float(c.get(f'{k} Component', 0)))) * 255) for k in ('Red', 'Green', 'Blue')]
    return '#' + ''.join(f'{v:02x}' for v in rgb)


def palette(path: Path) -> list[str]:
    d = plistlib.loads(path.read_bytes())
    bg = d['Background Color']
    fg = d['Foreground Color']
    fallback = {'Cursor Color': fg, 'Cursor Text Color': bg, 'Selection Color': d.get('Ansi 8 Color', fg),
                'Selected Text Color': fg}
    return [hex_of(d.get(k) or fallback[k]) for k in KEYS]


YAML_KEYS = ['background', 'foreground', 'cursor', 'cursor_text', 'selection', 'selection_text'] + [
    f'color_{i:02d}' for i in range(1, 17)]


def palette_yaml(path: Path) -> list[str]:
    d = yaml.safe_load(path.read_text())
    fallback = {'cursor': d['foreground'], 'cursor_text': d['background'], 'selection': d.get('color_09', d['foreground']),
                'selection_text': d['foreground']}
    return [str(d.get(k) or fallback[k]).lower() for k in YAML_KEYS]


def find_yaml(src: Path, name: str) -> Path | None:
    for f in (src / 'yaml').glob('*.yml'):
        if yaml.safe_load(f.read_text()).get('name') == name:
            return f
    return None


def main() -> None:
    src = Path(sys.argv[1])
    repo = Path(__file__).resolve().parent.parent
    licences = yaml.safe_load((src / 'licenses.yml').read_text())
    commit = subprocess.run(['git', '-C', str(src), 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()
    out, notes = [], []
    for name in sorted(licences, key=str.lower):
        entry = licences[name]
        if not isinstance(entry, dict) or not permissive(str(entry.get('license', ''))):
            continue
        f = src / 'schemes' / f'{name}.itermcolors'
        if f.exists():
            colours = palette(f)
        elif (y := find_yaml(src, name)) is not None:
            colours = palette_yaml(y)
        else:
            continue
        out.append([name, ' '.join(colours)])
        notes.append(f"| {name} | {entry['license']} | {entry.get('source') or ''} |")
    # The rest are listed by name only: Settings offers to download one from
    # the collection when someone asks, which is their own download.
    bundled = {n for n, _ in out}
    more = sorted((f.stem for f in (src / 'schemes').glob('*.itermcolors') if f.stem not in bundled), key=str.lower)
    (repo / 'src/lib/data/terminal-schemes.json').write_text(
        json.dumps({'source': f'https://github.com/mbadolato/iTerm2-Color-Schemes/tree/{commit}', 'schemes': out,
                    'more': more},
                   ensure_ascii=False, separators=(',', ':')) + '\n')
    (repo / 'src/lib/data/terminal-schemes-licenses.md').write_text(
        '# Bundled terminal colour schemes\n\n'
        'Taken from [iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes) '
        f'(commit `{commit[:12]}`), MIT licence, copyright (c) 2011 to present Mark Badolato. '
        'Each scheme keeps the licence of its own author, listed below; only schemes under a '
        'permissive licence are bundled. Regenerate with `scripts/gen-terminal-schemes.py`.\n\n'
        '| Scheme | Licence | Source |\n|---|---|---|\n' + '\n'.join(notes) + '\n')
    print(f'{len(out)} schemes, {len(more)} more by name')


if __name__ == '__main__':
    main()
