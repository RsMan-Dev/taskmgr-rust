#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QtQml/qqml.h>

#include "seriesplot.h"
#include <QPalette>
#include <QStyleHints>
#include <QSettings>
#include <QStandardPaths>
#include <QTranslator>
#include <QTimer>
#include <QSocketNotifier>

#include <csignal>
#include <sys/socket.h>
#include <unistd.h>

static int s_signalFds[2];

// Breeze colours for the portable build, which ships no KDE colour schemes.
static QPalette breezePalette(bool dark)
{
    const auto c = [](int r, int g, int b) { return QColor(r, g, b); };
    QPalette p;
    p.setColor(QPalette::Window, dark ? c(32, 35, 38) : c(239, 240, 241));
    p.setColor(QPalette::WindowText, dark ? c(252, 252, 252) : c(35, 38, 41));
    p.setColor(QPalette::Base, dark ? c(20, 22, 24) : c(255, 255, 255));
    p.setColor(QPalette::AlternateBase, dark ? c(29, 31, 34) : c(247, 247, 247));
    p.setColor(QPalette::Text, dark ? c(252, 252, 252) : c(35, 38, 41));
    p.setColor(QPalette::Button, dark ? c(41, 44, 48) : c(252, 252, 252));
    p.setColor(QPalette::ButtonText, dark ? c(252, 252, 252) : c(35, 38, 41));
    p.setColor(QPalette::Highlight, c(61, 174, 233));
    p.setColor(QPalette::HighlightedText, dark ? c(252, 252, 252) : c(255, 255, 255));
    p.setColor(QPalette::ToolTipBase, dark ? c(41, 44, 48) : c(247, 247, 247));
    p.setColor(QPalette::ToolTipText, dark ? c(252, 252, 252) : c(35, 38, 41));
    p.setColor(QPalette::Link, dark ? c(29, 153, 243) : c(41, 128, 185));
    p.setColor(QPalette::PlaceholderText, dark ? c(161, 169, 177) : c(112, 125, 138));
    for (auto role : {QPalette::Text, QPalette::WindowText, QPalette::ButtonText}) {
        p.setColor(QPalette::Disabled, role, dark ? c(161, 169, 177) : c(112, 125, 138));
    }
    return p;
}

static QPalette schemePalette(const QString &path)
{
    QSettings ini(path, QSettings::IniFormat);
    const auto color = [&ini](const QString &group, const QString &key) {
        const QStringList rgb = ini.value(group + QLatin1Char('/') + key).toStringList();
        return rgb.size() >= 3 ? QColor(rgb[0].toInt(), rgb[1].toInt(), rgb[2].toInt()) : QColor();
    };
    QPalette p;
    p.setColor(QPalette::Window, color(QStringLiteral("Colors:Window"), QStringLiteral("BackgroundNormal")));
    p.setColor(QPalette::WindowText, color(QStringLiteral("Colors:Window"), QStringLiteral("ForegroundNormal")));
    p.setColor(QPalette::Base, color(QStringLiteral("Colors:View"), QStringLiteral("BackgroundNormal")));
    p.setColor(QPalette::AlternateBase, color(QStringLiteral("Colors:View"), QStringLiteral("BackgroundAlternate")));
    p.setColor(QPalette::Text, color(QStringLiteral("Colors:View"), QStringLiteral("ForegroundNormal")));
    p.setColor(QPalette::Button, color(QStringLiteral("Colors:Button"), QStringLiteral("BackgroundNormal")));
    p.setColor(QPalette::ButtonText, color(QStringLiteral("Colors:Button"), QStringLiteral("ForegroundNormal")));
    p.setColor(QPalette::Highlight, color(QStringLiteral("Colors:Selection"), QStringLiteral("BackgroundNormal")));
    p.setColor(QPalette::HighlightedText, color(QStringLiteral("Colors:Selection"), QStringLiteral("ForegroundNormal")));
    p.setColor(QPalette::ToolTipBase, color(QStringLiteral("Colors:Tooltip"), QStringLiteral("BackgroundNormal")));
    p.setColor(QPalette::ToolTipText, color(QStringLiteral("Colors:Tooltip"), QStringLiteral("ForegroundNormal")));
    p.setColor(QPalette::Link, color(QStringLiteral("Colors:View"), QStringLiteral("ForegroundLink")));
    p.setColor(QPalette::PlaceholderText, color(QStringLiteral("Colors:View"), QStringLiteral("ForegroundInactive")));
    p.setColor(QPalette::Disabled, QPalette::Text, color(QStringLiteral("Colors:View"), QStringLiteral("ForegroundInactive")));
    p.setColor(QPalette::Disabled, QPalette::WindowText, color(QStringLiteral("Colors:Window"), QStringLiteral("ForegroundInactive")));
    p.setColor(QPalette::Disabled, QPalette::ButtonText, color(QStringLiteral("Colors:Button"), QStringLiteral("ForegroundInactive")));
    return p;
}

// Only write() is async-signal-safe: the event loop does the actual quit.
static void onSignal(int)
{
    char c = 1;
    (void)!write(s_signalFds[0], &c, 1);
}

static void quitOnSignals(QCoreApplication &app)
{
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, s_signalFds) != 0) {
        return;
    }
    auto *notifier = new QSocketNotifier(s_signalFds[1], QSocketNotifier::Read, &app);
    QObject::connect(notifier, &QSocketNotifier::activated, &app, [] {
        char c;
        (void)!read(s_signalFds[1], &c, 1);
        QCoreApplication::quit();
    });
    for (int sig : {SIGTERM, SIGINT, SIGHUP}) {
        std::signal(sig, onSignal);
    }
}

extern "C" int taskmgr_run(int argc, char **argv)
{
    static int s_argc = argc;
    QApplication app(s_argc, argv);
    app.setApplicationName(QStringLiteral("taskmgr"));
    app.setOrganizationName(QStringLiteral("rsman"));
    app.setOrganizationDomain(QStringLiteral("rsman.fr"));
    app.setApplicationDisplayName(QStringLiteral("Gestionnaire des tâches"));
    app.setDesktopFileName(QStringLiteral("fr.rsman.taskmgr-rust"));
    app.setWindowIcon(QIcon(QStringLiteral(":/icons/logo.svg")));
    QIcon::setFallbackThemeName(QStringLiteral("breeze"));

    // "prefs/theme": "light" or "dark" applies a Breeze colour scheme to
    // this application only, the way KColorSchemeManager does: Kirigami reads
    // the scheme from the property, widgets from the palette.
    const QString theme = QSettings().value(QStringLiteral("prefs/theme")).toString();
    // Portable build (AppImage): no KDE platform theme or style is shipped.
    const bool portable = qEnvironmentVariableIsSet("TASKMGR_PORTABLE");
    if (portable) {
        const bool dark = theme == QLatin1String("dark")
            || (theme.isEmpty() && QGuiApplication::styleHints()->colorScheme() == Qt::ColorScheme::Dark);
        // Fusion picks its own palette from the colour scheme.
        QGuiApplication::styleHints()->setColorScheme(dark ? Qt::ColorScheme::Dark : Qt::ColorScheme::Light);
        app.setPalette(breezePalette(dark));
        // The AppImage bundles the few Breeze icons Kirigami asks for by name.
        QStringList iconPaths = QIcon::themeSearchPaths();
        for (const QString &d : QStandardPaths::standardLocations(QStandardPaths::GenericDataLocation)) {
            iconPaths << d + QStringLiteral("/icons");
        }
        QIcon::setThemeSearchPaths(iconPaths);
        QIcon::setThemeName(dark ? QStringLiteral("breeze-dark") : QStringLiteral("breeze"));
    } else if (theme == QLatin1String("light") || theme == QLatin1String("dark")) {
        const QString scheme = QStandardPaths::locate(QStandardPaths::GenericDataLocation,
            theme == QLatin1String("light") ? QStringLiteral("color-schemes/BreezeLight.colors")
                                            : QStringLiteral("color-schemes/BreezeDark.colors"));
        if (!scheme.isEmpty()) {
            app.setProperty("KDE_COLOR_SCHEME_PATH", scheme);
            app.setPalette(schemePalette(scheme));
        }
    }

    // "prefs/language": empty follows the system, "en" or "fr" forces it.
    const QString language = QSettings().value(QStringLiteral("prefs/language")).toString();
    static QTranslator translator;
    const bool loaded = language.isEmpty()
        ? translator.load(QLocale(), QStringLiteral("taskmgr"), QStringLiteral("_"), QStringLiteral(":/i18n"))
        : translator.load(QStringLiteral(":/i18n/taskmgr_") + language);
    if (loaded) {
        QCoreApplication::installTranslator(&translator);
    }
    quitOnSignals(app);

    // org.kde.desktop draws with QStyle, hence QApplication. The portable
    // build ships styles/Fusion/Theme.qml in Kirigami so its colours follow
    // the palette.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
        QQuickStyle::setStyle(portable ? QStringLiteral("Fusion") : QStringLiteral("org.kde.desktop"));
    }

    qmlRegisterType<SeriesPlot>("org.taskmgr.charts", 1, 0, "SeriesPlot");
    QQmlApplicationEngine engine;
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app,
                     [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    // Development aid: TASKMGR_PAGE / TASKMGR_SELECT pick the view,
    // TASKMGR_GRAB saves the window to a file after a few seconds and quits.
    QVariantMap initial;
    if (!qEnvironmentVariableIsEmpty("TASKMGR_PAGE")) {
        initial.insert(QStringLiteral("currentPage"), qEnvironmentVariableIntValue("TASKMGR_PAGE"));
    }
    if (!qEnvironmentVariableIsEmpty("TASKMGR_SELECT")) {
        initial.insert(QStringLiteral("initialSelection"), qEnvironmentVariable("TASKMGR_SELECT"));
    }
    engine.setInitialProperties(initial);
    engine.loadFromModule("org.taskmgr", "Main");

    const QString grab = qEnvironmentVariable("TASKMGR_GRAB");
    if (!grab.isEmpty()) {
        QTimer::singleShot(qEnvironmentVariableIntValue("TASKMGR_GRAB_DELAY") * 1000 + 8000, &app, [&engine, grab] {
            if (auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().value(0))) {
                window->grabWindow().save(grab);
            }
            QCoreApplication::quit();
        });
        if (auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().value(0));
            window && !qEnvironmentVariableIsEmpty("TASKMGR_GRAB_HEIGHT")) {
            const int w = qEnvironmentVariableIntValue("TASKMGR_GRAB_WIDTH");
            window->resize(w > 0 ? w : window->width(), qEnvironmentVariableIntValue("TASKMGR_GRAB_HEIGHT"));
        }
    }
    return app.exec();
}
