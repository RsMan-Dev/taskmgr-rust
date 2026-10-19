import QtQuick

// List of string keys updated in place (moves, inserts, removals), so views
// keep their delegates and scroll position when the data refreshes.
ListModel {
    id: model

    function sync(keys) {
        for (let i = 0; i < keys.length; i++) {
            if (i < count && get(i).key === keys[i]) {
                continue
            }
            let j = -1
            for (let k = i + 1; k < count; k++) {
                if (get(k).key === keys[i]) {
                    j = k
                    break
                }
            }
            if (j >= 0) {
                move(j, i, 1)
            } else {
                insert(i, { key: keys[i] })
            }
        }
        if (count > keys.length) {
            remove(keys.length, count - keys.length)
        }
    }
}
