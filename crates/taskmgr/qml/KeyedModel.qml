import QtQuick

// List of string keys updated in place (moves, inserts, removals), so views
// keep their delegates and scroll position when the data refreshes.
ListModel {
    id: model

    // Mirror of the keys: ListModel.get() builds an object per call, far
    // too slow to search hundreds of rows every second.
    property var current: []

    function sync(keys) {
        const wanted = new Set(keys)
        for (let i = current.length - 1; i >= 0; i--) {
            if (!wanted.has(current[i])) {
                remove(i)
                current.splice(i, 1)
            }
        }
        for (let i = 0; i < keys.length; i++) {
            if (current[i] === keys[i]) {
                continue
            }
            const j = current.indexOf(keys[i], i + 1)
            if (j >= 0) {
                move(j, i, 1)
                current.splice(i, 0, current.splice(j, 1)[0])
            } else {
                insert(i, { key: keys[i] })
                current.splice(i, 0, keys[i])
            }
        }
    }
}
