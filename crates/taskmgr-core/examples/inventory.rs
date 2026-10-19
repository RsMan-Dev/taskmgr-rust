//! Prints the hardware inventory as an indented tree.
fn print(n: &taskmgr_core::inventory::Node, depth: usize) {
    let value = n
        .text
        .clone()
        .or_else(|| n.number.map(|v| format!("{v} {}", n.unit.unwrap_or(""))));
    println!(
        "{}{}{}",
        "  ".repeat(depth),
        n.label,
        value.map(|v| format!(": {v}")).unwrap_or_default()
    );
    for c in &n.children {
        print(c, depth + 1);
    }
}

fn main() {
    let root = taskmgr_core::Root::system();
    let mut sampler = taskmgr_core::Sampler::new(root.clone());
    let snapshot = sampler.sample();
    let info = taskmgr_core::system_info(&root);
    for n in taskmgr_core::inventory::collect(&root, &snapshot, &info) {
        print(&n, 0);
    }
}
