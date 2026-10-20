pragma Singleton

import QtQuick

// Rows of the process list: sections, groups, and inside an open group its
// processes as a tree. Keys: "s:<kind>", "g:<group>", "p:<group>:<pid>".
QtObject {
    function build(groups, sections, query, cmp, expanded, collapsed) {
        const keys = []
        const data = {}
        if (!groups) {
            return { keys: keys, data: data }
        }
        const roleRank = { normal: 0, infra: 1, wrapper: 2 }
        const matches = p => p.name.toLowerCase().includes(query) || String(p.pid) === query
        for (const [kind, title] of sections) {
            let list = groups.filter(g => g.kind === kind)
            if (query !== "") {
                list = list.filter(g => g.name.toLowerCase().includes(query) || g.processes.some(matches))
            }
            if (list.length === 0) {
                continue
            }
            keys.push("s:" + kind)
            data["s:" + kind] = { type: "section", title: title, count: list.length }
            list.sort(cmp)
            for (const g of list) {
                const gk = "g:" + g.key
                keys.push(gk)
                data[gk] = { type: "group", group: g }
                const open = expanded[g.key] || (query !== "" && g.processes.some(matches))
                if (!open || g.processes.length < 2) {
                    continue
                }
                // A tree inside the group: each process under its parent.
                const members = new Set(g.processes.map(p => p.pid))
                const children = {}
                const roots = []
                for (const p of g.processes) {
                    if (members.has(p.ppid) && p.ppid !== p.pid) {
                        (children[p.ppid] = children[p.ppid] ?? []).push(p)
                    } else {
                        roots.push(p)
                    }
                }
                const order = (a, b) => roleRank[a.role ?? "normal"] - roleRank[b.role ?? "normal"] || cmp(a, b)
                const walk = (list, depth) => {
                    for (const p of list.sort(order)) {
                        const pk = "p:" + g.key + ":" + p.pid
                        const kids = children[p.pid] ?? []
                        keys.push(pk)
                        data[pk] = { type: "process", proc: p, group: g, depth: depth, branch: kids.length > 0 }
                        if (kids.length > 0 && (query !== "" || !collapsed[pk])) {
                            walk(kids, depth + 1)
                        }
                    }
                }
                walk(roots, 0)
            }
        }
        return { keys: keys, data: data }
    }
}
