#include "app.h"

#include <QApplication>
#include <QDir>
#include <QFile>
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

#include <string>
#include <vector>

static int s_signalFds[2];

// The shipped colour scheme as a file: KColorScheme reads a path.
static QString schemeFile(bool dark)
{
    const QString name = dark ? QStringLiteral("dark.colors") : QStringLiteral("light.colors");
    QFile in(QStringLiteral(":/colors/") + name);
    const QString dir = QStandardPaths::writableLocation(QStandardPaths::CacheLocation);
    QDir().mkpath(dir);
    const QString path = dir + QLatin1Char('/') + name;
    QFile out(path);
    if (in.open(QIODevice::ReadOnly)) {
        const QByteArray data = in.readAll();
        if (!(out.open(QIODevice::ReadOnly) && out.readAll() == data)) {
            out.close();
            if (out.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                out.write(data);
            }
        }
    }
    return path;
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

int taskmgr_run(const rust::Vec<rust::String> &args)
{
    // QApplication keeps argc and argv for its whole life.
    static std::vector<std::string> s_args(args.begin(), args.end());
    static std::vector<char *> s_argv;
    for (auto &a : s_args) {
        s_argv.push_back(a.data());
    }
    s_argv.push_back(nullptr);
    static int s_argc = static_cast<int>(s_args.size());
    // The desktop's platform theme would bring its own palette, fonts,
    // icons and style; the portal one only gives file dialogs and the
    // light or dark preference.
    qputenv("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
    qunsetenv("QT_STYLE_OVERRIDE");
    QApplication app(s_argc, s_argv.data());
    app.setApplicationName(QStringLiteral("taskmgr"));
    app.setOrganizationName(QStringLiteral("rsman"));
    app.setOrganizationDomain(QStringLiteral("rsman.fr"));
    app.setApplicationDisplayName(QStringLiteral("Gestionnaire des tâches"));
    app.setDesktopFileName(QStringLiteral("fr.rsman.taskmgr-rust"));
    app.setWindowIcon(QIcon(QStringLiteral(":/icons/logo.svg")));
    QIcon::setFallbackThemeName(QStringLiteral("breeze"));

    // One look everywhere: Breeze controls and colours shipped with the
    // app, whatever the desktop's style or colour scheme. Only light or dark
    // follows the desktop, unless the settings choose.
    const QString theme = QSettings().value(QStringLiteral("prefs/theme")).toString();
    const bool dark = theme == QLatin1String("dark")
        || (theme.isEmpty() && QGuiApplication::styleHints()->colorScheme() == Qt::ColorScheme::Dark);
    const QString scheme = schemeFile(dark);
    app.setProperty("KDE_COLOR_SCHEME_PATH", scheme);
    app.setPalette(schemePalette(scheme));
    QStringList iconPaths = QIcon::themeSearchPaths();
    for (const QString &d : QStandardPaths::standardLocations(QStandardPaths::GenericDataLocation)) {
        iconPaths << d + QStringLiteral("/icons");
    }
    QIcon::setThemeSearchPaths(iconPaths);
    QIcon::setThemeName(dark ? QStringLiteral("breeze-dark") : QStringLiteral("breeze"));

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

    QQuickStyle::setStyle(QStringLiteral("org.kde.breeze"));

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
