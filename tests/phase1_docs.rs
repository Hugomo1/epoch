use std::fs;
use std::path::Path;

fn read(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    fs::read_to_string(&full)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", full.display()))
}

#[test]
fn agents_root_declares_product_spec_canonical() {
    let root = read("AGENTS.md");
    assert!(
        root.contains("Canonical source: `PRODUCT_SPEC.md`."),
        "AGENTS.md must declare PRODUCT_SPEC.md as canonical source"
    );
    assert!(
        root.contains("Conflict rule: `PRODUCT_SPEC.md` (what) > `AGENTS.md` (how)."),
        "AGENTS.md must include explicit conflict rule"
    );
}

#[test]
fn agents_subguides_reference_product_spec() {
    let ui = read("src/ui/AGENTS.md");
    let parsers = read("src/parsers/AGENTS.md");
    let collectors = read("src/collectors/AGENTS.md");

    for (path, content) in [
        ("src/ui/AGENTS.md", ui),
        ("src/parsers/AGENTS.md", parsers),
        ("src/collectors/AGENTS.md", collectors),
    ] {
        assert!(
            content.contains("PRODUCT_SPEC.md"),
            "{path} must reference PRODUCT_SPEC.md"
        );
        assert!(
            content.contains("PRODUCT_SPEC.md` (what) > `AGENTS.md` (how)"),
            "{path} must include local conflict rule"
        );
    }
}

#[test]
fn agents_conflict_rule_is_consistent() {
    let root = read("AGENTS.md");
    let ui = read("src/ui/AGENTS.md");
    let parsers = read("src/parsers/AGENTS.md");
    let collectors = read("src/collectors/AGENTS.md");

    let expected = "PRODUCT_SPEC.md` (what) > `AGENTS.md` (how)";
    assert!(
        root.contains(expected),
        "root AGENTS conflict rule mismatch"
    );
    assert!(ui.contains(expected), "ui AGENTS conflict rule mismatch");
    assert!(
        parsers.contains(expected),
        "parsers AGENTS conflict rule mismatch"
    );
    assert!(
        collectors.contains(expected),
        "collectors AGENTS conflict rule mismatch"
    );

    let disallowed = "AGENTS.md is canonical source";
    assert!(
        !root.contains(disallowed),
        "root AGENTS must not invert source-of-truth precedence"
    );
}
