# /// script
# requires-python = "==3.14.*"
# dependencies = ["tree-sitter==0.25.2", "tree-sitter-rust==0.24.2"]
# ///

import argparse
import collections
import os
import posixpath
import re
import subprocess
import sys
import unittest
from contextlib import chdir
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from tempfile import TemporaryDirectory

from tree_sitter import Language, Parser
import tree_sitter_rust

CAPS = {"file": 800, "body": 60, "branches": 12, "depth": 3,
        "args": 5, "reason": 0, "inline-test": 0, "pub": 40}
BASELINE = Path("scripts/clean-code-baseline.tsv")
BOOTSTRAP_BASE = "04b57d012e7fe50da78ff94738d5c0ac6fb6c21b"
HEADER = "rule\tpath\titem\tceiling\n"
ATTR = {"attribute_item", "inner_attribute_item"}
COMMENT = {"line_comment", "block_comment"}
CONTROL = {"if_expression", "match_expression", "for_expression",
           "while_expression", "loop_expression"}
FN = {"function_item", "function_signature_item"}
SRC = re.compile(r"(?:crates/[^/]+|packages/browser-client/runtime)/(src|tests)/.*\.rs$")


def git(*args):
    return subprocess.check_output(("git", *args))


def text(node):
    return node.text.decode() if node else ""


def field(node, key):
    return node.child_by_field_name(key)


def row(node):
    return node.start_point.row + 1


def rows(node):
    return range(row(node), node.end_point.row + bool(node.end_point.column) + 1)


def groups(node):
    pending = []
    for child in node.named_children:
        if child.type == "attribute_item" or (
            child.type in COMMENT and text(child).startswith(("///", "/**"))
        ):
            pending.append(child)
        elif child.type not in COMMENT:
            yield pending, child
            pending = []


def parts(tree):
    group, result = [], []
    for token in tree.children[1:-1] if tree else []:
        if token.type == ",":
            if group:
                result.append(group)
            group = []
        elif token.type not in COMMENT:
            group.append(token)
    return result + ([group] if group else [])


def meta(tokens):
    return text(tokens[0]), tokens[1] if len(tokens) > 1 else None


def attrs(nodes):
    return [a.named_child(0) for a in nodes if a.type in ATTR]


def possible(tokens):
    name, args = meta(tokens)
    if tokens[-1].type == "token_tree" and name in {"all", "any", "not"}:
        values = [possible(part) for part in parts(args)]
        if name == "all":
            return all(v[0] for v in values), any(v[1] for v in values)
        if name == "any":
            return any(v[0] for v in values), all(v[1] for v in values)
        return values[0][1], values[0][0]
    return (False, True) if len(tokens) == 1 and name == "test" else (True, True)


def test_fn(nodes):
    return any(text(a.named_child(0)) == "test" for a in attrs(nodes))


def test_only(nodes):
    for a in attrs(nodes):
        name, args = meta(a.named_children)
        if name == "test" or (
            name == "cfg" and not possible(parts(args)[0])[0]
        ):
            return True
    return False


def production_view(node, excluded, omitted):
    for tags, child in groups(node):
        if test_only(tags):
            excluded.update([child, *tags])
            for skip in [child, *tags]:
                omitted.update(rows(skip))
            if test_fn(tags) or (child.type == "mod_item" and field(child, "body")):
                yield (max(rows(child)) - row(tags[0]) + 1, child)
        else:
            yield from production_view(child, excluded, omitted)


def descendants(node, excluded=frozenset()):
    if node in excluded:
        return
    yield node
    for child in node.named_children:
        yield from descendants(child, excluded)


def entries(node, scope=(), testing=False):
    testing |= test_only([n for n in node.named_children if n.type == "inner_attribute_item"])
    for tags, child in groups(node):
        child_test = testing or test_only(tags)
        yield tags, child, scope, child_test
        if child.type == "mod_item" and field(child, "body"):
            yield from entries(field(child, "body"), scope + (text(field(child, "name")),), child_test)


def links(path, root, inventory):
    p = PurePosixPath(path)
    for tags, node, scope, testing in entries(root):
        if node.type != "mod_item" or field(node, "body"):
            continue
        name = text(field(node, "name"))
        override = next((field(a, "value") for a in attrs(tags)
                         if text(a.named_child(0)) == "path"), None)
        folder = p.parent if p.name in {"lib.rs", "main.rs", "mod.rs"} else p.with_suffix("")
        folder = p.parent if p.parent.name == "bin" else folder
        normal = folder.joinpath(*scope)
        candidates = [normal / f"{name}.rs", normal / name / "mod.rs"]
        if override:
            base = normal if scope else p.parent
            candidates = [base / text(override.named_child(0))]
        found = [posixpath.normpath(c) for c in candidates if posixpath.normpath(c) in inventory]
        if len(found) != 1:
            raise ValueError((path, row(node), candidates, found))
        yield found[0], testing


def compact(node):
    if node.type in COMMENT:
        return ""
    if node.type in {"string_literal", "raw_string_literal"} or not node.child_count:
        return text(node)
    return "".join(map(compact, node.children))


def item_scope(node, tags, owner):
    if node.type == "impl_item":
        name = compact(field(node, "type"))
        if trait := field(node, "trait"):
            name = f"<{name} as {compact(trait)}>"
        else:
            name = f"<{name}>"
    elif node.type.endswith("_item") or node.type in {"field_declaration", "enum_variant"}:
        name = text(field(node, "name"))
    else:
        return owner
    if not name:
        return owner
    conditions = sorted(compact(a) for a in attrs(tags) if text(a.named_child(0)) == "cfg")
    if conditions:
        name += "[" + ",".join(conditions) + "]"
    return f"{owner}::{name}" if owner else name


def locations(node, tags=(), owner=""):
    owner = item_scope(node, tags, owner)
    yield node, tags, owner
    for child_tags, child in groups(node):
        yield from locations(child, child_tags, owner)


def suppression(name, args, conditions=()):
    arguments = parts(args)
    if name == "cfg_attr":
        predicate = "".join(map(compact, arguments[0]))
        for branch in arguments[1:]:
            yield from suppression(*meta(branch), (*conditions, predicate))
    if name not in {"allow", "expect"}:
        return
    reasons = [part for part in arguments if text(part[0]) == "reason"]
    if any(
        len(part) == 3 and text(part[1]) == "="
        and part[2].type in {"string_literal", "raw_string_literal"}
        and any(text(x).strip() for x in part[2].named_children if x.type == "string_content")
        for part in reasons
    ):
        return
    lints = sorted("".join(map(compact, part)) for part in arguments if part not in reasons)
    condition = f" cfg_attr({','.join(conditions)})" if conditions else ""
    yield f"{name}({','.join(lints)}){condition}"


def flow(node, excluded, depth=0):
    if node in excluded or node.type in FN or node.type == "token_tree":
        return 0, depth
    control = node.type in CONTROL or (
        node.type == "let_declaration" and field(node, "alternative") is not None
    )
    nested = depth + int(control)
    choices = nested if control else 0
    if node.type == "match_expression":
        choices += max(0, len([a for a in field(node, "body").named_children
                               if a.type == "match_arm"]) - 1)
    if node.type == "match_pattern" and field(node, "condition"):
        choices += 1
    if node.type in {"binary_expression", "let_chain"}:
        choices += sum(c.type in {"&&", "||"} for c in node.children)
    peak = nested
    for child in node.named_children:
        child_score, child_peak = flow(child, excluded, nested)
        choices += child_score
        peak = max(peak, child_peak)
    return choices, peak


def code_rows(node, lines, excluded):
    if node in excluded or node.type in COMMENT:
        return set()
    if not node.child_count:
        return {i for i in rows(node) if lines[i - 1].strip()}
    return set().union(*(code_rows(c, lines, excluded) for c in node.children))


def source_files(revision):
    command = ("ls-tree", "-rz", "--name-only", revision) if revision else (
        "ls-files", "--cached", "-z"
    )
    paths = sorted(p for p in git(*command).decode().split("\0") if SRC.fullmatch(p))
    if not revision:
        return {path: Path(path).read_bytes() for path in paths}
    query = "".join(f"{revision}:{p}\n" for p in paths).encode()
    raw = subprocess.run(("git", "cat-file", "--batch"), input=query,
                         stdout=subprocess.PIPE, check=True).stdout
    files = {}
    offset = 0
    for path in paths:
        end = raw.index(b"\n", offset)
        _, kind, size = raw[offset:end].split()
        if kind != b"blob":
            raise ValueError(f"{revision}:{path}: expected a Git blob")
        offset = end + 1
        files[path] = raw[offset:offset + int(size)]
        offset += int(size) + 1
    return files


@dataclass(frozen=True)
class Finding:
    value: int
    line: int


def measure(files):
    if not files:
        raise ValueError("no tracked Rust source to measure")
    parser = Parser(Language(tree_sitter_rust.language()))
    data = {}
    for path, source in files.items():
        root = parser.parse(source).root_node
        if root.has_error:
            bad = next(n for n in descendants(root) if n.is_error or n.is_missing)
            raise ValueError(f"{path}:{row(bad)}: could not parse Rust source")
        lines = source.split(b"\n")
        if lines[-1] == b"":
            lines.pop()
        data[path] = (root, lines)

    paths = data.keys()
    src = {p for p in paths if SRC.fullmatch(p)[1] == "src"}
    edges = {p: list(links(p, data[p][0], src)) for p in src}
    incoming = {target for targets in edges.values() for target, _ in targets}
    gated_files = {p for p in src if test_only(
        [n for n in data[p][0].named_children if n.type == "inner_attribute_item"])}
    production = src - incoming - gated_files
    queue = list(production)
    while queue:
        for target, testing in edges[queue.pop()]:
            if not testing and target not in production and target not in gated_files:
                production.add(target)
                queue.append(target)

    if not production:
        raise ValueError("no production Rust source to measure")

    found = {}
    for path, (root, lines) in data.items():
        def put(rule, value, line, key=""):
            if value <= CAPS[rule]:
                return
            identity = (rule, path, key)
            if identity in found:
                raise ValueError(f"{path}:{line}: ambiguous {rule} identity {key}")
            found[identity] = Finding(value, line)

        omitted, excluded, test_items = set(), set(), {}
        if path in production:
            test_items = {node: span for span, node in production_view(root, excluded, omitted)}
            put("file", len(lines) - len(omitted - code_rows(root, lines, excluded)), 1)
            nodes = descendants(root, excluded)
            put("pub", sum(n.type == "visibility_modifier" and text(n) == "pub" for n in nodes), 1)

        production_nodes = set(descendants(root, excluded)) if path in production else set()
        suppression_counts = collections.Counter()
        suppression_lines = {}
        for node, tags, owner in locations(root):
            statements = attrs(tags)
            if node.type == "inner_attribute_item":
                statements.append(node.named_child(0))
            for attribute in statements:
                for missing in suppression(*meta(attribute.named_children)):
                    key = f"{owner} {missing}" if owner else missing
                    suppression_counts[key] += 1
                    suppression_lines.setdefault(key, row(attribute))
            if node in test_items:
                put("inline-test", test_items[node], row(node), owner)
            if node not in production_nodes or node.type not in FN:
                continue
            parameters = field(node, "parameters")
            put("args", sum(n not in excluded and n.type in {
                "parameter", "self_parameter", "variadic_parameter"
            } for n in parameters.named_children), row(node), owner)
            if (body := field(node, "body")) is not None:
                put("body", len(code_rows(body, lines, excluded)), row(node), owner)
                score, depth = flow(body, excluded)
                put("branches", score, row(node), owner)
                put("depth", depth, row(node), owner)
        for key, count in suppression_counts.items():
            put("reason", count, suppression_lines[key], key)
    return found


def render(ceilings):
    return (HEADER + "".join(
        f"{rule}\t{path}\t{key}\t{limit}\n"
        for (rule, path, key), limit in sorted(ceilings.items())
    )).encode()


def read_ceilings(snapshot):
    ceilings = {}
    for line in snapshot.decode().splitlines()[1:]:
        rule, path, item, value = line.split("\t")
        if int(value) <= CAPS[rule]:
            raise ValueError(f"baseline row is already within its limit: {line}")
        ceilings[rule, path, item] = int(value)
    if render(ceilings) != snapshot:
        raise ValueError("base TSV must contain sorted, unique violation rows")
    return ceilings


def ceilings_for(findings):
    return {key: finding.value for key, finding in findings.items()}


def regressions(ceilings, candidate):
    failures = []
    for identity, finding in sorted(candidate.items()):
        rule, path, key = identity
        limit = ceilings.get(identity, CAPS[rule])
        if finding.value > limit:
            failures.append(f"{path}:{finding.line}: {rule} {key or '(file)'}: "
                            f"{finding.value} exceeds ceiling {limit}")
    return failures


def check(base, write):
    base = git("rev-parse", "--verify", f"{base}^{{commit}}").decode().strip()
    baseline_path = BASELINE.as_posix()
    if git("ls-tree", "-z", base, "--", baseline_path):
        ceilings = read_ceilings(git("show", f"{base}:{baseline_path}"))
    elif base == BOOTSTRAP_BASE:
        ceilings = ceilings_for(measure(source_files(base)))
    else:
        raise ValueError(f"{base}: missing {baseline_path}")
    candidate = measure(source_files(None))
    if failures := regressions(ceilings, candidate):
        print("\n".join(failures), file=sys.stderr)
        return 1
    expected = render(ceilings_for(candidate))
    if write:
        BASELINE.write_bytes(expected)
    elif BASELINE.read_bytes() != expected:
        raise ValueError(f"{baseline_path} is stale; use --write-baseline with the same --base")
    print(f"Clean Code ratchet passed ({len(candidate)} baseline rows).")
    return 0


class CheckerTests(unittest.TestCase):
    path = "crates/fixture/src/lib.rs"

    def oversized(self, name="legacy", statements=63):
        return (f"fn {name}() {{\n" + "    let _ = 1;\n" * statements + "}\n").encode()

    def findings(self, source):
        return measure({self.path: source})

    def test_unchanged_shifted_and_reduced(self):
        source = self.oversized()
        base = ceilings_for(self.findings(source))
        self.assertEqual(len(base), 1)
        for candidate in (source, b"\n" + source, self.oversized(statements=62)):
            self.assertEqual(regressions(base, self.findings(candidate)), [])

    def test_rejects_ceiling_increase(self):
        base = ceilings_for(self.findings(self.oversized()))
        failures = regressions(base, self.findings(self.oversized(statements=64)))
        self.assertEqual(len(failures), 1)
        self.assertIn("66 exceeds ceiling 65", failures[0])

    def test_rejects_swapped_violation(self):
        base = ceilings_for(self.findings(self.oversized()))
        failures = regressions(base, self.findings(self.oversized(name="replacement")))
        self.assertEqual(len(failures), 1)
        self.assertIn("replacement: 65 exceeds ceiling 60", failures[0])

    def test_rejects_stale_row(self):
        with TemporaryDirectory() as folder, chdir(folder):
            git("init", "-q")
            BASELINE.parent.mkdir()
            source = Path(self.path)
            source.parent.mkdir(parents=True)
            source.write_bytes(self.oversized())
            BASELINE.write_bytes(render(ceilings_for(self.findings(source.read_bytes()))))
            git("add", "--", BASELINE.as_posix(), self.path)
            git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null",
                "commit", "-qm", "fixture baseline")
            source.write_bytes(b"fn clean() {}\n")
            with self.assertRaisesRegex(ValueError, "is stale"):
                check("HEAD", False)

    def test_rejects_reasonless_inactive_cfg_attr(self):
        source = b'#[cfg_attr(any(), allow(dead_code))]\nfn small() {}\n'
        failures = regressions({}, self.findings(source))
        self.assertEqual(len(failures), 1)
        self.assertIn("allow(dead_code) cfg_attr(any())", failures[0])
        self.assertEqual(self.findings(source.replace(
            b"allow(dead_code)", b'allow(dead_code, reason = "Host-only entry point")'
        )), {})

    def test_test_only_module_control(self):
        module = "crates/fixture/src/unit.rs"
        main = b"#[cfg(test)]\nmod unit;\n"
        self.assertEqual(measure({self.path: main, module: self.oversized()}), {})
        inline = b"#[cfg(test)]\nmod unit { #[test] fn test_case() {} }\n"
        self.assertEqual(set(self.findings(inline)), {
            ("inline-test", self.path, "unit[cfg(test)]")
        })

    def test_shared_module_counts_as_production(self):
        module = "crates/fixture/src/shared.rs"
        main = b"#[cfg(test)]\nmod shared;\n#[cfg(not(test))]\nmod shared;\n"
        self.assertEqual(
            ceilings_for(measure({self.path: main, module: self.oversized()})),
            {("body", module, "legacy"): 65},
        )


def main():
    parser = argparse.ArgumentParser(description="Check Rust source against the Clean Code ratchet.")
    parser.add_argument("--base", help="Git base commit used for violation ceilings")
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if (not args.base and not args.self_test) or (args.write_baseline and not args.base):
        parser.error("--base is required for checking or writing the baseline")
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CheckerTests)
        if not unittest.TextTestRunner().run(suite).wasSuccessful():
            return 1
    if not args.base:
        return 0
    try:
        os.chdir(git("rev-parse", "--show-toplevel").decode().strip())
        return check(args.base, args.write_baseline)
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"Clean Code ratchet: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
