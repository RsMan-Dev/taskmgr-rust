pragma Singleton

import QtQuick

// Interface icons are bundled (Fluent UI System Icons, MIT) so they look the
// same under every desktop theme. Names not listed here, such as application
// icons, still come from the system theme.
QtObject {
    readonly property string logo: "qrc:/icons/logo.svg"

    readonly property var files: ({
        "nav-processes": "apps_list",
        "nav-performance": "data_area",
        "nav-details": "text_bullet_list_ltr",
        "nav-startup": "rocket",
        "nav-services": "puzzle_piece",
        "nav-users": "people",
        "nav-connections": "globe",
        "nav-crashes": "bug",
        "journal": "document_text",
        "tree": "text_bullet_list_tree",
        "nav-system": "desktop",
        "nav-settings": "settings",
        "menu": "navigation",
        "minimize": "arrow_minimize",
        "maximize": "arrow_maximize",
        "pin": "pin",
        "end-task": "dismiss_circle",
        "kill": "prohibited",
        "suspend": "pause",
        "resume": "play",
        "start": "play",
        "stop": "record_stop",
        "restart": "arrow_counterclockwise",
        "refresh": "arrow_clockwise",
        "copy": "copy",
        "copied": "checkmark",
        "enable": "add_circle",
        "disable": "subtract_circle",
        "admin": "shield_keyhole",
        "open-folder": "folder_open",
        "hide": "eye_off",
        "expand": "chevron_right",
        "collapse": "chevron_down",
        "sort-up": "arrow_sort_up",
        "sort-down": "arrow_sort_down",
        "search": "search",
        "info": "info",
        "warning": "warning",
        "cpu": "developer_board",
        "memory": "ram",
        "disk": "hard_drive",
        "network": "plug_connected",
        "wifi": "wifi_1",
        "gpu": "developer_board_lightning",
        "sensor": "temperature",
        "computer": "desktop_tower",
        "board": "board",
        "os": "desktop",
        "app": "app_generic",
        "background": "window_settings",
        "system": "shield",
    })

    // Light ink on dark backgrounds, dark ink otherwise, picked from the
    // text colour where the icon is drawn.
    function of(name, color) {
        const f = files[name]
        if (!f) {
            return name
        }
        const c = Qt.color(color ?? "#232629")
        const variant = (0.299 * c.r + 0.587 * c.g + 0.114 * c.b) > 0.5 ? "on-dark" : "on-light"
        return "qrc:/icons/" + variant + "/" + f + "_20_regular.svg"
    }
}
