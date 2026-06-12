#!/usr/bin/env python3
"""Transpile ELK's generated *Options.java metadata into Rust.

Parses the generated option classes (e.g. CoreOptions.java, LayeredOptions.java)
from the ELK 0.11.0 sources and emits, per module:
  - `Property` statics with exact ids and defaults,
  - option metadata registrations (id, type, targets, legacy ids, parser)
    used by the Rust LayoutMetaDataService.

Java enums referenced by options are generated separately via gen_enums().
Usage: gen_options.py <module.json>   (see tools/genconf/*.json)
"""

import json
import re
import sys
from pathlib import Path

SRC_ROOT = Path(__file__).parent / "elk-sources"


def camel_to_shouty(name):
    return name  # option constant names are already SHOUTY_SNAKE in Java


def parse_default_constants(java: str) :
    """NAME_DEFAULT = <expr>; including multiline expressions."""
    consts = {}
    for m in re.finditer(
        r"private static final [^=;]+?(\w+(?:_DEFAULT|_LOWER_BOUND|_UPPER_BOUND))\s*=\s*(.+?);\n",
        java,
        re.DOTALL,
    ):
        consts[m.group(1)] = re.sub(r"\s+", " ", m.group(2).strip())
    return consts


def parse_properties(java):
    """Yield (const_name, java_type, id, default_const, lower, upper)."""
    pat = re.compile(
        r"public static final IProperty<([^=]+?)>\s+(\w+)\s*=\s*new Property<[^=]+?>\(\s*"
        r'"([^"]+)"\s*(?:,\s*(\w+|null)\s*)?(?:,\s*(\w+|null)\s*)?(?:,\s*(\w+|null)\s*)?\)',
        re.DOTALL,
    )
    for m in pat.finditer(java):
        jtype = re.sub(r"\s+", "", m.group(1))
        yield (m.group(2), jtype, m.group(3), m.group(4), m.group(5), m.group(6))


def parse_registrations(java):
    """Yield dicts for each LayoutOptionData.Builder registration."""
    for m in re.finditer(
        r"registry\.register\(new LayoutOptionData\.Builder\(\)(.*?)\.create\(\)\s*\)",
        java,
        re.DOTALL,
    ):
        body = m.group(1)
        entry = {}
        for call in re.finditer(r"\.(\w+)\(((?:[^()]|\([^()]*\))*)\)", body):
            entry.setdefault(call.group(1), call.group(2))
        yield entry


JAVA_TYPE_TO_RUST = {
    "Boolean": "bool",
    "Integer": "i32",
    "Double": "f64",
    "String": "String",
    "KVector": "KVector",
    "KVectorChain": "KVectorChain",
    "ElkPadding": "ElkPadding",
    "ElkMargin": "ElkMargin",
    "IndividualSpacings": "IndividualSpacings",
}


def rust_type(jtype: str, conf) :
    jtype = conf.get("type_aliases", {}).get(jtype, jtype)
    if jtype in JAVA_TYPE_TO_RUST:
        return JAVA_TYPE_TO_RUST[jtype]
    m = re.match(r"EnumSet<(\w+)>", jtype)
    if m:
        return f"EnumSet<{m.group(1)}>"
    if jtype in conf.get("enums", {}):
        return jtype
    return None  # internal/unsupported type: skipped


def rust_default(expr: str, rtype: str, conf) :
    """Translate a Java default-value expression to Rust."""
    expr = expr.strip()
    overrides = conf.get("default_overrides", {})
    if expr in overrides:
        return overrides[expr]
    if expr == "null":
        return None
    if rtype == "bool":
        m = re.fullmatch(r"Boolean\.valueOf\((\w+)\)", expr)
        return m.group(1) if m else expr
    if rtype == "i32":
        return expr
    if rtype == "f64":
        # Java often writes integral doubles as e.g. `20`
        if re.fullmatch(r"-?\d+", expr):
            return f"{expr}.0"
        if re.fullmatch(r"-?[\d.eE+-]+[fd]?", expr):
            stripped = expr.rstrip("fdFD")
            return f"{stripped}.0" if re.fullmatch(r"-?\d+", stripped) else stripped
        return expr
    if rtype == "String":
        if expr.startswith('"'):
            return f"{expr}.to_string()"
        return None
    if rtype == "KVector":
        m = re.fullmatch(r"new KVector\(([^,]+),([^)]+)\)", expr)
        if m:
            return f"KVector::new({float(m.group(1))}f64, {float(m.group(2))}f64)"
        if expr == "new KVector()":
            return "KVector::default()"
    if rtype in ("ElkPadding", "ElkMargin"):
        m = re.fullmatch(r"new (?:ElkPadding|ElkMargin)\(\)", expr)
        if m:
            return "Spacing::default()"
        m = re.fullmatch(r"new (?:ElkPadding|ElkMargin)\(([^,)]+)\)", expr)
        if m:
            return f"Spacing::uniform({float(m.group(1))}f64)"
        m = re.fullmatch(r"new (?:ElkPadding|ElkMargin)\(([^,)]+),([^,)]+)\)", expr)
        if m:
            return f"Spacing::of_lr_tb({float(m.group(2))}f64, {float(m.group(1))}f64)"
        m = re.fullmatch(
            r"new (?:ElkPadding|ElkMargin)\(([^,)]+),([^,)]+),([^,)]+),([^,)]+)\)", expr
        )
        if m:
            vals = ", ".join(f"{float(g)}f64" for g in m.groups())
            return f"Spacing::new({vals})"
    m = re.match(r"EnumSet<(\w+)>", rtype)
    if m:
        enum = m.group(1)
        if expr.startswith("EnumSet.noneOf"):
            return "EnumSet::none()"
        if expr.startswith("EnumSet.allOf"):
            return "EnumSet::all()"
        em = re.fullmatch(r"EnumSet\.of\(([^)]+)\)", expr)
        if em:
            items = ", ".join(
                f"{enum}::{v.split('.')[-1]}" for v in em.group(1).split(",")
            )
            return f"EnumSet::of(&[{items}])"
        return None
    # plain enum constant: Alignment.AUTOMATIC
    m = re.fullmatch(r"(\w+)\.(\w+)", expr)
    if m and rtype == m.group(1):
        return f"{rtype}::{m.group(2)}"
    return None


def parse_value_kind(reg_type: str, option_class: str) :
    """Map metadata type to the Rust OptionKind used for string parsing."""
    t = reg_type.split(".")[-1]
    if t == "STRING":
        return 'OptionKind::Str'
    if t == "BOOLEAN":
        return 'OptionKind::Bool'
    if t == "INT":
        return 'OptionKind::Int'
    if t == "DOUBLE":
        return 'OptionKind::Double'
    if t == "ENUM":
        return f'OptionKind::Enum(parse_enum::<{option_class.split(".")[-1]}>)'
    if t == "ENUMSET":
        return f'OptionKind::EnumSet(parse_enumset::<{option_class.split(".")[-1]}>)'
    if t == "OBJECT":
        parser = {
            "KVector": "OptionKind::Object(parse_kvector)",
            "KVectorChain": "OptionKind::Object(parse_kvectorchain)",
            "ElkPadding": "OptionKind::Object(parse_padding)",
            "ElkMargin": "OptionKind::Object(parse_margin)",
            "IndividualSpacings": "OptionKind::Object(parse_individual_spacings)",
        }.get(option_class.split(".")[-1])
        return parser or "OptionKind::Unparseable"
    return "OptionKind::Unparseable"


def gen_enum(java_file: Path) :
    java = java_file.read_text()
    m = re.search(r"public enum (\w+)[^{]*\{", java)
    if not m:
        return None
    name = m.group(1)
    body = java[m.end():]
    # variants run until the first ';' or the closing brace of the enum if
    # there are no members. Strip comments first.
    body = re.sub(r"/\*.*?\*/", "", body, flags=re.DOTALL)
    body = re.sub(r"//[^\n]*", "", body)
    stop = len(body)
    for i, ch in enumerate(body):
        if ch in ";}":
            stop = i
            break
    variants = []
    for part in body[:stop].split(","):
        part = part.strip()
        vm = re.match(r"^(\w+)", part)
        if vm:
            variants.append(vm.group(1))
    if not variants:
        return None
    lines = [f"elk_enum! {{", f"    pub enum {name} {{"]
    for v in variants:
        lines.append(f"        {v},")
    lines.append("    }")
    lines.append("}")
    return "\n".join(lines)


def generate(conf_path: str):
    conf = json.loads(Path(conf_path).read_text())
    out = [
        "// GENERATED by tools/gen_options.py from ELK 0.11.0 sources — do not edit.",
        "#![allow(non_camel_case_types)]",
        "",
        conf.get("header", ""),
    ]

    # enums
    for enum_name, enum_src in conf.get("enums", {}).items():
        if enum_src == "manual":
            continue
        code = gen_enum(SRC_ROOT / enum_src)
        if code is None:
            print(f"WARN: could not parse enum {enum_name} from {enum_src}", file=sys.stderr)
            continue
        out.append(code)
        out.append("")

    skipped = []
    for options_src in conf["options_files"]:
        java = (SRC_ROOT / options_src).read_text()
        consts = parse_default_constants(java)
        regs = {r.get("id", "").strip('"'): r for r in parse_registrations(java)}

        props = []
        for name, jtype, oid, default_const, _lo, _hi in parse_properties(java):
            rtype = rust_type(jtype, conf)
            if rtype is None:
                skipped.append((name, jtype))
                continue
            default_expr = None
            if default_const and default_const != "null":
                jexpr = consts.get(default_const, default_const)
                default_expr = rust_default(jexpr, rtype, conf)
                if default_expr is None:
                    print(f"WARN: no default translation for {name}: {jexpr}", file=sys.stderr)
            if default_expr is None:
                props.append(
                    f'pub static {name}: Property<{rtype}> = Property::new("{oid}");'
                )
            else:
                props.append(
                    f'pub static {name}: Property<{rtype}> = '
                    f'Property::with_default("{oid}", || {default_expr});'
                )
        out.extend(props)
        out.append("")

        # registration function
        fn_name = conf["register_fn"]
        out.append(f"pub fn {fn_name}(reg: &mut LayoutMetaDataRegistry) {{")
        for oid, r in regs.items():
            kind = parse_value_kind(r.get("type", ""), r.get("optionClass", "").replace(".class", ""))
            targets = re.findall(r"Target\.(\w+)", r.get("targets", ""))
            tflags = " | ".join(f"Targets::{t}" for t in targets) or "Targets::empty()"
            legacy = re.findall(r'"([^"]+)"', r.get("legacyIds", ""))
            legacy_rust = (
                "&[" + ", ".join(f'"{l}"' for l in legacy) + "]" if legacy else "&[]"
            )
            group = r.get("group", '""')
            out.append(
                f'    reg.register_option(OptionData {{ id: "{oid}", '
                f"group: {group}, kind: {kind}, targets: {tflags}, legacy_ids: {legacy_rust} }});"
            )
        out.append("}")
        out.append("")

    if skipped:
        out.append("// Skipped internal/unsupported-type properties:")
        for name, jtype in skipped:
            out.append(f"//   {name}: {jtype}")

    Path(conf["output"]).write_text("\n".join(out) + "\n")
    print(f"wrote {conf['output']}")


if __name__ == "__main__":
    generate(sys.argv[1])
