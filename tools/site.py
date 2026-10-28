"""Build website/index.html: every lesson's guide, source and expected output
as one page, from the files in this repository.

    python tools/site.py

Run it again after changing a lesson, a guide or website/template.html. The
page is self-contained apart from its web fonts, which fall back to system
faces offline.
"""

import html, re
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "website" / "index.html"

def esc(s): return html.escape(s, quote=False)

# ---------------------------------------------------------------- highlight
RS_KW = set("as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while".split())
RS_TY = set("i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char str".split())
RS_DEF_AFTER = {"fn", "struct", "enum", "trait", "type", "mod", "macro_rules!"}
RS_TOK = re.compile(r"""
 (?P<com>//[^\n]*|/\*.*?\*/)
|(?P<attr>\#!?\[[^\]\n]*\])
|(?P<str>b?"(?:\\.|[^"\\])*")
|(?P<chr>b?'(?:\\.|[^'\\\n])')
|(?P<life>'[A-Za-z_]\w*)
|(?P<num>0x[0-9A-Fa-f_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?(?:_?(?:[ui](?:8|16|32|64|128|size)|f32|f64))?)
|(?P<mac>[A-Za-z_]\w*!(?!=))
|(?P<id>[A-Za-z_]\w*)
|(?P<op>[\[\]{}()<>&*|^~!=+\-/%.,;:?@$#]+)
|(?P<ws>\s+)
|(?P<etc>.)
""", re.S | re.X)

def hl_rust(src):
    out, prev = [], None
    for m in RS_TOK.finditer(src):
        kind, text = m.lastgroup, m.group()
        t = esc(text)
        if kind == "com": out.append(f'<span class="c-com">{t}</span>')
        elif kind == "attr": out.append(f'<span class="c-attr">{t}</span>')
        elif kind in ("str", "chr"): out.append(f'<span class="c-str">{t}</span>')
        elif kind == "num": out.append(f'<span class="c-num">{t}</span>')
        elif kind == "life": out.append(f'<span class="c-ty">{t}</span>')
        elif kind == "mac": out.append(f'<span class="c-mac">{t}</span>')
        elif kind == "id":
            if prev in RS_DEF_AFTER and text not in RS_KW: out.append(f'<span class="c-def">{t}</span>')
            elif text in RS_KW: out.append(f'<span class="c-kw">{t}</span>')
            elif text in RS_TY or re.match(r"[A-Z][a-z0-9]\w*$", text): out.append(f'<span class="c-ty">{t}</span>')
            else: out.append(t)
        else: out.append(t)
        if kind not in ("ws", "com"): prev = text
    return "".join(out)

PY_KW = set("from import def return if elif else for in not and or while with as class try except finally raise pass None True False lambda is continue break".split())
PY_TOK = re.compile(r'''(?P<com>\#[^\n]*)|(?P<str>"""(?:.|\n)*?"""|"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*')|(?P<num>\b\d+\b)|(?P<id>[A-Za-z_]\w*)|(?P<ws>\s+)|(?P<etc>.)''', re.S)
def hl_py(src):
    out = []
    for m in PY_TOK.finditer(src):
        kind, t = m.lastgroup, esc(m.group())
        if kind == "com": out.append(f'<span class="c-com">{t}</span>')
        elif kind == "str": out.append(f'<span class="c-str">{t}</span>')
        elif kind == "num": out.append(f'<span class="c-num">{t}</span>')
        elif kind == "id" and m.group() in PY_KW: out.append(f'<span class="c-kw">{t}</span>')
        else: out.append(t)
    return "".join(out)

def hl_toml(src):
    out = []
    for line in src.split("\n"):
        if line.lstrip().startswith("#"): out.append(f'<span class="c-com">{esc(line)}</span>')
        elif line.startswith("["): out.append(f'<span class="c-kw">{esc(line)}</span>')
        else:
            m = re.match(r'(\s*[\w.-]+\s*=\s*)(.*)', line)
            out.append(esc(m.group(1)) + f'<span class="c-str">{esc(m.group(2))}</span>' if m else esc(line))
    return "\n".join(out)

def highlight(src, lang):
    lang = lang.split(" ")[0]
    if lang == "rust": return hl_rust(src)
    if lang == "py": return hl_py(src)
    if lang == "toml": return hl_toml(src)
    return esc(src)

def code_block(src, lang):
    lines = highlight(src, lang).split("\n")
    if lines and lines[-1] == "": lines.pop()
    rows = "".join(f'<span class="ln"><span class="no">{i}</span>{l or " "}</span>' for i, l in enumerate(lines, 1))
    return f'<pre class="code"><code>{rows}</code></pre>'

def snippet(src, lang):
    return '<pre class="snip"><code>' + highlight(src, lang) + "</code></pre>"

# ---------------------------------------------------------------- markdown
LINKS = {
    "0_from_kayan.md": "#from-kayan", "1_aos_vs_soa.md": "#aos-vs-soa", "2_archetype_table.md": "#archetype-table",
    "3_many_archetypes.md": "#many-archetypes", "4_entity_handles.md": "#entity-handles", "5_inspector.md": "#inspector",
    "6_kayan.md": "#kayan", "7_access_patterns.md": "#access-patterns", "LICENSE": "#tools",
}
def link_target(url):
    base = url.split("/")[-1]
    if url.startswith("http"): return url, True
    if base in LINKS: return LINKS[base], False
    if url.endswith("docs/README.md") and not url.endswith("ecs/docs/README.md"): return "#advanced", False
    if base == "README.md": return "#overview", False
    m = re.match(r"(\d)\.(\d)_\w+\.md$", base)
    if m: return f"#ecs-{m.group(1)}-{m.group(2)}", False
    return url, False

def inline(text):
    parts = re.split(r"(`[^`]+`)", text)
    out = []
    for p in parts:
        if p.startswith("`") and p.endswith("`") and len(p) > 1:
            out.append(f"<code>{esc(p[1:-1])}</code>"); continue
        p = esc(p)
        p = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", p)
        p = re.sub(r"(?<![\w*])\*([^*\s](?:[^*]*?[^*\s])?)\*(?![\w*])", r"<em>\1</em>", p)
        def lk(m):
            href, ext = link_target(html.unescape(m.group(2)))
            attrs = ' target="_blank" rel="noopener"' if ext else ""
            return f'<a href="{esc(href)}"{attrs}>{m.group(1)}</a>'
        p = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", lk, p)
        out.append(p)
    # links whose text is code: [`x`](y) were split; fix simple case
    s = "".join(out)
    s = re.sub(r"\[(<code>[^<]*</code>)\]\(([^)]+)\)", lambda m: f'<a href="{esc(link_target(m.group(2))[0])}">{m.group(1)}</a>', s)
    return s

def cells(row):
    # Split a table row on its pipes, but not on `\|`, which is a pipe in a cell.
    return [c.strip().replace("\\|", "|") for c in re.split(r"(?<!\\)\|", row.strip().strip("|"))]

def fence(lines, i, indent):
    """A fenced block starting at lines[i]: (lang, source, next line)."""
    lang = lines[i].strip()[3:].strip(); block = []; i += 1
    while i < len(lines) and lines[i].strip() != "```":
        block.append(lines[i][indent:] if lines[i][:indent].strip() == "" else lines[i]); i += 1
    return lang, "\n".join(block), i + 1

def md(text, skip_title=False):
    lines = text.split("\n")
    out, para, i = [], [], 0
    def flush():
        nonlocal para
        if para: out.append("<p>" + inline(" ".join(l.strip() for l in para)) + "</p>"); para = []
    while i < len(lines):
        line = lines[i]
        if not line.strip(): flush(); i += 1; continue
        if line.startswith("```"):
            flush(); lang, src, i = fence(lines, i, 0)
            out.append(snippet(src, lang)); continue
        h = re.match(r"(#{1,4}) (.*)", line)
        if h:
            flush(); lvl = len(h.group(1))
            if not (skip_title and lvl == 1):
                out.append(f"<h{lvl+1}>{inline(h.group(2))}</h{lvl+1}>")
            i += 1; continue
        if line.startswith("    ") and not para:
            block = []
            while i < len(lines) and (lines[i].startswith("    ") or not lines[i].strip()):
                block.append(lines[i][4:]); i += 1
            while block and not block[-1].strip(): block.pop()
            out.append(snippet("\n".join(block), "")); continue
        if line.startswith("|"):
            flush(); rows = []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append(cells(lines[i])); i += 1
            head, body = rows[0], [r for r in rows[2:]]
            t = '<div class="tablewrap"><table><thead><tr>' + "".join(f"<th>{inline(c)}</th>" for c in head) + "</tr></thead><tbody>"
            for r in body: t += "<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>"
            out.append(t + "</tbody></table></div>"); continue
        lm = re.match(r"(\s*)(-|\d+\.) (.*)", line)
        if lm and not para:
            ordered = lm.group(2) != "-"
            # An item is a list of parts: runs of text, and fenced blocks
            # indented under it. A blank line followed by an indented line
            # stays in the item.
            items = []
            while i < len(lines):
                m2 = re.match(r"(\s*)(-|\d+\.) (.*)", lines[i])
                if m2: items.append([["text", m2.group(3)]]); i += 1; continue
                if not items: break
                if lines[i].strip().startswith("```") and lines[i].startswith("  "):
                    indent = len(lines[i]) - len(lines[i].lstrip())
                    lang, src, i = fence(lines, i, indent)
                    items[-1].append(["code", lang, src]); continue
                if lines[i].startswith("  ") and lines[i].strip():
                    if items[-1][-1][0] == "text": items[-1][-1][1] += " " + lines[i].strip()
                    else: items[-1].append(["text", lines[i].strip()])
                    i += 1; continue
                if not lines[i].strip():
                    j = i
                    while j < len(lines) and not lines[j].strip(): j += 1
                    if j < len(lines) and lines[j].startswith("   ") and not re.match(r"\s*(-|\d+\.) ", lines[j]):
                        items[-1].append(["text", ""]); i = j; continue
                break
            def part(p):
                if p[0] == "code": return snippet(p[2], p[1])
                return f"<p>{inline(p[1])}</p>" if p[1] else ""
            def item(parts):
                if len(parts) == 1: return inline(parts[0][1])
                return "".join(part(p) for p in parts)
            tag = "ol" if ordered else "ul"
            out.append(f"<{tag}>" + "".join("<li>" + item(it) + "</li>" for it in items) + f"</{tag}>"); continue
        para.append(line); i += 1
    flush()
    return "\n".join(out)

# ---------------------------------------------------------------- content
lessons = [
    ("aos-vs-soa", "1", "AoS vs SoA", "src/1.aos_vs_soa/aos_vs_soa", "docs/1_aos_vs_soa.md", "The same particles in three layouts, and what SIMD needs"),
    ("archetype-table", "2", "One archetype table", "src/2.archetype_table/archetype_table", "docs/2_archetype_table.md", "Columns, rows, spawn, swap-remove, systems over slices"),
    ("many-archetypes", "3", "Many archetypes", "src/3.many_archetypes/many_archetypes", "docs/3_many_archetypes.md", "One table type for every archetype: ids, signatures, queries"),
    ("entity-handles", "4", "Entity handles", "src/4.entity_handles/entity_handles", "docs/4_entity_handles.md", "Generational handles, slots, moving between archetypes"),
    ("inspector", "5", "The inspector", "src/5.inspector/inspector", "docs/5_inspector.md", "Printing and editing any entity from reflect records"),
    ("kayan", "6", "kayan", "src/6.kayan/kayan", "docs/6_kayan.md", "A small game, with structural changes deferred to a command buffer"),
    ("access-patterns", "7", "Layout follows access", "src/7.access_patterns/access_patterns", "docs/7_access_patterns.md", "Four layouts under streams and lookups, and how an ECS decides"),
]

def read(p): return (REPO / p).read_text(encoding="utf-8")
def nlines(s): return s.count("\n")

views = []

def navitem(tok, num, title, blurb):
    n = f'<span class="num">{num}</span>' if num else '<span class="num dim">·</span>'
    return f'<a class="navlink" href="#{tok}" data-view="{tok}">{n}<span class="nt"><span class="ttl">{esc(title)}</span><span class="blurb">{esc(blurb)}</span></span></a>'

def group(heading, items):
    return f'<div class="group"><h2>{esc(heading)}</h2>' + "".join(navitem(*n) for n in items) + "</div>"

def program_view(tok, eyebrow, title, base, guide, unit, before_code=""):
    src = read(base + ".rs"); exp = read(base + ".expected")
    return f'''<section class="view" id="v-{tok}" data-view="{tok}" hidden>
<header class="vhead"><p class="eyebrow">{eyebrow}</p><h1>{esc(title)}</h1><p class="path">{base}.rs</p></header>
<div class="tabs" role="tablist" aria-label="{esc(title)} views">
  <button role="tab" type="button" class="tab" aria-selected="true" data-pane="guide">Guide</button>
  <button role="tab" type="button" class="tab" aria-selected="false" data-pane="code">Code <span class="count">{nlines(src)} lines</span></button>
  <button role="tab" type="button" class="tab" aria-selected="false" data-pane="output">Output <span class="count">{nlines(exp)} lines</span></button>
</div>
<div class="pane" data-pane="guide"><div class="prose">{md(guide, skip_title=True)}</div></div>
<div class="pane" data-pane="code" hidden>{before_code}{code_block(src, "rust")}</div>
<div class="pane" data-pane="output" hidden><p class="note">What <code>{base.split("/")[-1]}</code> prints. <code>tools/check.py</code> fails the {unit} if a single byte differs.</p><pre class="term"><code>{esc(exp)}</code></pre></div>
</section>'''

# ---- Start: the overview is the course's front page
nav_start = [('overview', '', 'Overview', 'Build an ECS: the course, and how to build it')]
views.append(f'''<section class="view" id="v-overview" data-view="overview">
<header class="vhead"><p class="eyebrow">Rust · data-oriented design · build an ECS</p><h1>rust-dod</h1>
<p class="lede">Build an archetype entity component system from nothing, in fifteen chapters you read, compile and step through: from an entity as a number to a game running on it.</p>
<p class="lede"><a href="#ecs-1-1">Start with 1.1, an entity is a number</a></p></header>
<div class="prose">{md(read("README.md"), skip_title=True)}</div>
</section>''')

# ---- Build an ECS: the chapters, grouped by part
ecs_lessons = []
for doc in sorted((REPO / "ecs" / "docs").glob("*.md")):
    m = re.match(r"(\d)\.(\d)_(\w+)\.md$", doc.name)
    part, step = m.group(1), m.group(2)
    title = read(f"ecs/docs/{doc.name}").split("\n", 1)[0].lstrip("# ").split(" ", 1)[1]
    found = list((REPO / "ecs" / "src").glob(f"{part}.*/{part}.{step}.*/*.rs"))
    base = found[0].relative_to(REPO).as_posix()[:-3]
    ecs_lessons.append((f"ecs-{part}-{step}", f"{part}.{step}", title, base, f"ecs/docs/{doc.name}", part))
ECS_PARTS = {"1": "Entities", "2": "Components", "3": "Systems"}
# Each chapter's one-line summary, from the "about" column of the README's table.
ecs_about = {m.group(1): re.sub(r"`", "", m.group(2)).strip()
             for m in re.finditer(r"^\| (\d\.\d) \| [^|]+ \| ([^|]+) \|$", read("README.md"), re.M)}

nav_ecs = {part: [] for part in ECS_PARTS}
for tok, num, title, base, doc, part in ecs_lessons:
    guide = re.sub(r"^`ecs/src/[^`]+`\n\n", "", read(doc), count=1, flags=re.M)  # the header shows the path
    nav_ecs[part].append((tok, num, title, ecs_about.get(num, "")))
    views.append(program_view(tok, f"Build an ECS · part {part}, {ECS_PARTS[part].lower()} · {num}", title, base, guide, "chapter"))

# ---- Advanced: layout and SIMD, the seven lessons
nav_adv = [('advanced', '', 'About these lessons', 'Memory, aliasing and SIMD, for after the course')]
diagram = '''
<figure class="memmap" aria-label="One 68-byte particle against 64-byte cache lines">
  <figcaption><span class="eyebrow">Lesson 1, to scale</span><span>One <code>Particle</code> is 68 bytes. Movement reads the first 24.</span></figcaption>
  <div class="strip aos">
    <span class="f use" style="--b:12" title="position, 12 bytes">position</span><span class="f use" style="--b:12" title="velocity, 12 bytes">velocity</span><span class="f" style="--b:16" title="tint, 16 bytes">tint</span><span class="f" style="--b:4" title="age, 4 bytes"></span><span class="f" style="--b:4" title="lifetime, 4 bytes"></span><span class="f" style="--b:4" title="size, 4 bytes"></span><span class="f" style="--b:16" title="name, 16 bytes">name</span>
  </div>
  <div class="ruler"><span style="--at:0">0</span><span style="--at:24">24</span><span style="--at:64">64, end of a cache line</span><span style="--at:68">68</span></div>
  <div class="legend"><span><i class="sw use"></i>read by movement: position 12, velocity 12</span><span><i class="sw"></i>fetched anyway: tint 16, age 4, lifetime 4, size 4, name 16</span></div>
  <p class="figtext">In the array of structs, every cache line movement pulls in is about two thirds tint, age, life, size and name. In the struct of arrays, movement fetches only the position and velocity columns: <strong>6,291,456</strong> bytes a step instead of <strong>17,825,792</strong>.</p>
</figure>'''
views.append(f'''<section class="view" id="v-advanced" data-view="advanced" hidden>
<header class="vhead"><p class="eyebrow">Advanced · seven lessons</p><h1>Layout and SIMD</h1><p class="path">docs/README.md</p></header>
{diagram}
<div class="prose">{md(read("docs/README.md"), skip_title=True)}</div>
</section>''')

nav_adv.append(('from-kayan', '0', 'From kayan', 'What was kept from the C kayan, and why'))
views.append(f'''<section class="view" id="v-from-kayan" data-view="from-kayan" hidden>
<header class="vhead"><p class="eyebrow">Advanced · lesson 0 · review</p><h1>From kayan</h1><p class="path">docs/0_from_kayan.md</p></header>
<div class="prose">{md(read("docs/0_from_kayan.md"), skip_title=True)}</div></section>''')

for tok, num, title, base, doc, blurb in lessons:
    nav_adv.append((tok, num, title, blurb))
    imports = '<p class="note">Brings in the world from <a href="#library"><code>includes/kayan.rs</code></a>.</p>' if "includes/kayan.rs" in read(base + ".rs") else ""
    views.append(program_view(tok, f"Advanced · lesson {num}", title, base, read(doc), "lesson", imports))

# ---- Source
nav_src = []
lib = read("includes/kayan.rs")
nav_src.append(('library', '', 'includes/kayan.rs', 'The world lessons 5 and 6 bring in'))
views.append(f'''<section class="view" id="v-library" data-view="library" hidden>
<header class="vhead"><p class="eyebrow">Advanced · library · built in lesson 4</p><h1>kayan.rs</h1><p class="path">includes/kayan.rs · {nlines(lib)} lines</p></header>
{code_block(lib, "rust")}</section>''')

tools_html = ""
for p, lang in (("tools/check.py", "py"), ("tools/check_snippets.py", "py"), ("Cargo.toml", "toml"), (".gitignore", "txt"), ("LICENSE", "txt")):
    s = read(p)
    tools_html += f'<h2 class="filehead">{p}</h2>' + code_block(s, lang)
nav_src.append(('tools', '', 'Build and check', 'check.py, check_snippets.py, Cargo.toml, license'))
views.append(f'''<section class="view" id="v-tools" data-view="tools" hidden>
<header class="vhead"><p class="eyebrow">Tooling</p><h1>Build and check</h1><p class="path">tools/check.py runs every program and diffs its output</p></header>
{tools_html}</section>''')

nav_html = (group("Start", nav_start)
            + "".join(group(f"Part {part} · {name}", nav_ecs[part]) for part, name in ECS_PARTS.items())
            + group("Advanced · layout and SIMD", nav_adv)
            + group("Source", nav_src))

template = (REPO / "website" / "template.html").read_text(encoding="utf-8")
page = template.replace("{{NAV}}", nav_html).replace("{{VIEWS}}", "\n".join(views))
OUT.write_text(page, encoding="utf-8", newline="\n")
print(f"wrote {OUT.relative_to(REPO).as_posix()} ({len(page)//1024} KB)")
